# types

## Purpose

Core type system for the traits platform. Defines the cross-language type hierarchy (`TraitType`), runtime values (`TraitValue`), trait signatures, wire protocol messages, HTTP API types, and supported languages.

## Exports

* `TraitType` — enum of supported types (Int, Float, String, Bool, Bytes, Null, List, Map, Optional, Any, Handle)
* `TraitValue` — runtime value enum with JSON conversion and type matching
* `ParamDef` — parameter definition in a trait signature
* `ReturnDef` — return type definition
* `TraitSignature` — full trait signature (params + returns)
* `WorkerRequest` — wire protocol request to worker
* `WorkerResponse` — wire protocol response from worker (supports streaming)
* `CallRequest` — HTTP API call request
* `CallResponse` — HTTP API call response
* `Language` — supported implementation languages enum

---

## TraitType

### Purpose

Enum representing the cross-language type system. Used for type checking at call boundaries.

### Variants

* `Int` — integer
* `Float` — floating point
* `String` — text string
* `Bool` — boolean
* `Bytes` — raw byte array
* `Null` — null/none
* `List(Box<TraitType>)` — homogeneous list with element type
* `Map(Box<TraitType>, Box<TraitType>)` — map with key and value types
* `Optional(Box<TraitType>)` — nullable wrapper
* `Any` — untyped/dynamic, matches anything
* `Handle` — opaque handle to a non-serializable object

### Notes

* Serializes lowercase via `#[serde(rename_all = "lowercase")]`
* `List` serializes as `"list"`, `Map` as `"map"`, `Optional` as `"optional"`

---

## TraitValue

### Purpose

Runtime value that can be passed across language boundaries. Supports JSON round-tripping and type checking against `TraitType`.

### Variants

* `Null`
* `Bool(bool)`
* `Int(i64)`
* `Float(f64)`
* `String(String)`
* `List(Vec<TraitValue>)`
* `Map(HashMap<String, TraitValue>)`
* `Bytes(Vec<u8>)`

### Notes

* Uses `#[serde(untagged)]` — deserialized by trying each variant in order

---

## TraitValue::to_json

### Purpose

Convert a TraitValue to a `serde_json::Value`.

### Inputs

* `&self`

### Outputs

* `serde_json::Value`

### State

reads: self
writes: none

### Side Effects

none

### Dependencies

* `serde_json`

### Flow

1. Match on variant:
   - Null → Value::Null
   - Bool → Value::Bool
   - Int → Value::from (i64)
   - Float → from_f64, fallback to Null if NaN/Inf
   - String → Value::String
   - List → Value::Array (recursive)
   - Map → Value::Object (recursive)
   - Bytes → hex-encoded string

### Edge Cases

* Float NaN or Infinity converts to Null
* Bytes become a concatenated hex string (no separators)

### Example

```rust
TraitValue::Int(42).to_json() // => json!(42)
TraitValue::Bytes(vec![0xAB, 0xCD]).to_json() // => json!("abcd")
```

---

## TraitValue::from_json

### Purpose

Convert a `serde_json::Value` to TraitValue.

### Inputs

* `val`: `&serde_json::Value`

### Outputs

* `TraitValue`

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

* `serde_json`

### Flow

1. Match on JSON type:
   - Null → TraitValue::Null
   - Bool → TraitValue::Bool
   - Number: try i64 first, then f64, else Null
   - String → TraitValue::String
   - Array → TraitValue::List (recursive)
   - Object → TraitValue::Map (recursive)

### Edge Cases

* Number that is neither i64 nor f64 (shouldn't happen in practice) becomes Null

---

## TraitValue::is_handle

### Purpose

Check if value is a handle reference.

### Inputs

* `&self`

### Outputs

* `bool` — true if this is a Map containing a `__handle__` key

### Flow

1. Check if value is Map and contains key `"__handle__"`

---

## TraitValue::handle_id

### Purpose

Extract the handle ID string from a handle value.

### Inputs

* `&self`

### Outputs

* `Option<&str>` — the handle ID if this is a handle

### Flow

1. If Map with `__handle__` key containing a String, return that string
2. If String starting with `"hdl:"`, return the whole string
3. Otherwise None

### Edge Cases

* Map with `__handle__` key but non-String value returns None

---

## TraitValue::handle_language

### Purpose

Extract the language prefix from a handle ID.

### Inputs

* `&self`

### Outputs

* `Option<&str>` — language string (e.g. "py", "js")

### Flow

1. Get handle_id
2. Split by `:` into at most 3 parts
3. If parts are `["hdl", lang, id]`, return `lang`
4. Otherwise None

### Edge Cases

* Handle IDs without three colon-separated parts return None

### Example

```rust
// "hdl:py:abc123" -> Some("py")
// "hdl:js:xyz" -> Some("js")
```

---

## TraitValue::matches_type

### Purpose

Check if a runtime value matches a declared type. Used at call boundaries for validation.

### Inputs

* `&self` — the value
* `expected`: `&TraitType` — the declared type

### Outputs

* `bool`

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. `Any` matches everything
2. Handle values match `Handle` and `Any`
3. `Null` matches `Optional(_)` and `Null`
4. Non-null values match `Optional(inner)` if they match inner
5. Direct type matches: Bool-Bool, Int-Int, Float-Float, String-String, Bytes-Bytes
6. `Int` also matches `Float` (widening coercion)
7. `String` also matches `List(_)` (will be coerced by splitting on comma)
8. `List(items)` matches `List(inner)` if all items match inner
9. `Map(entries)` matches `Map(_, v_type)` if all values match v_type
10. Everything else: false

### Edge Cases

* Int matches Float (implicit widening)
* String matches List (for comma-separated input coercion)
* Handle values only match Handle or Any, not other types

### Example

```rust
assert!(TraitValue::Int(42).matches_type(&TraitType::Any));     // true
assert!(TraitValue::Int(42).matches_type(&TraitType::Float));   // true (widening)
assert!(!TraitValue::Int(42).matches_type(&TraitType::String)); // false
```

---

## TraitValue::type_name

### Purpose

Get a human-readable type name string for error messages.

### Inputs

* `&self`

### Outputs

* `&'static str` — "null", "bool", "int", "float", "string", "bytes", "list", "map", or "handle"

### Flow

1. If is_handle, return "handle"
2. Otherwise match variant to name string

---

## ParamDef

### Purpose

Parameter definition in a trait signature.

### Fields

* `name`: String — parameter name
* `param_type`: TraitType — declared type (serialized as "type")
* `description`: String — human-readable description (default: "")
* `optional`: bool — whether the parameter is optional (default: false)
* `pipe`: bool — when true, accepts piped stdin if not provided as CLI arg (default: false)

---

## ReturnDef

### Purpose

Return type definition for a trait signature.

### Fields

* `return_type`: TraitType — declared return type (serialized as "type")
* `description`: String — human-readable description (default: "")

---

## TraitSignature

### Purpose

A trait's full function signature.

### Fields

* `params`: Vec<ParamDef> — ordered parameter definitions
* `returns`: ReturnDef — return type info

---

## WorkerRequest

### Purpose

Wire protocol message sent from router to worker process.

### Fields

* `call`: String — trait path to invoke
* `args`: Vec<TraitValue> — arguments
* `id`: String — request correlation ID
* `stream`: bool — when true, worker sends chunked responses (default: false)

---

## WorkerResponse

### Purpose

Wire protocol message from worker to router. Supports three modes: regular result, streaming chunk, and stream end.

### Fields

* `id`: String — correlation ID
* `result`: Option<TraitValue> — final result (regular response)
* `error`: Option<String> — error message
* `chunk`: Option<TraitValue> — streaming chunk (serialized as `__chunk__`)
* `stream_end`: Option<bool> — stream termination signal (serialized as `__stream_end__`)

### Notes

* Regular response: `{ id, result }` or `{ id, error }`
* Streaming chunk: `{ id, __chunk__: value }`
* Stream end: `{ id, __stream_end__: true }`

---

## CallRequest

### Purpose

HTTP API request body for calling a trait.

### Fields

* `args`: serde_json::Value — positional array or named object (default: null)
* `interface_overrides`: Option<HashMap<String, String>> — per-call interface routing
* `trait_overrides`: Option<HashMap<String, String>> — per-call trait redirections

### Notes

* Named args accept both underscore and hyphen forms ("telegram_token" or "telegram-token")

---

## CallResponse

### Purpose

HTTP API response body from a trait call.

### Fields

* `result`: Option<serde_json::Value> — success result
* `error`: Option<String> — error message

---

## Language

### Purpose

Enum of supported implementation languages.

### Variants

* `Rust`
* `Python`
* `JavaScript`
* `TypeScript`
* `Java`
* `Perl`
* `Lisp`

### Notes

* Serializes lowercase via `#[serde(rename_all = "lowercase")]`
* Display trait formats the same way (e.g. `Language::Rust` → "rust")

---

## Internal Structure

The file defines a layered type system:
1. **Type declarations** (`TraitType`) — static type schema
2. **Runtime values** (`TraitValue`) — the actual data flowing through the system
3. **Signature types** (`ParamDef`, `ReturnDef`, `TraitSignature`) — trait interface contracts
4. **Wire protocol** (`WorkerRequest`, `WorkerResponse`) — communication with language workers
5. **HTTP API** (`CallRequest`, `CallResponse`) — external REST interface
6. **Language enum** — supported implementation languages

## Notes

* `TraitValue` deserialization is untagged — order of enum variants matters (Null tried last)
* `is_false` helper suppresses `stream: false` from WorkerRequest serialization
* Handle references use a special Map with `__handle__` key or String with `hdl:` prefix
