---
sidebar_position: 6
---

# Type System

traits.build has a cross-language type system that maps between Rust types and JSON representations.

## TraitType

The type declaration used in `.trait.toml` signatures:

| Type | TOML value | JSON Schema | Rust type |
|------|-----------|-------------|-----------|
| `string` | `"string"` | `string` | `String` |
| `int` | `"int"` | `integer` (int64) | `i64` |
| `float` | `"float"` | `number` (double) | `f64` |
| `bool` | `"bool"` | `boolean` | `bool` |
| `bytes` | `"bytes"` | `string` (binary) | `Vec<u8>` |
| `null` | `"null"` | `null` | `()` |
| `any` | `"any"` | no constraints | dynamic |
| `handle` | `"handle"` | opaque reference | `String` |
| `list<T>` | e.g. `"list<string>"` | `array` | `Vec<T>` |
| `map<K,V>` | e.g. `"map<string,int>"` | `object` | `HashMap<K,V>` |
| `T?` | e.g. `"string?"` | nullable | `Option<T>` |

## TraitValue

The runtime value enum used when traits execute:

```rust
pub enum TraitValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    List(Vec<TraitValue>),
    Map(HashMap<String, TraitValue>),
    Bytes(Vec<u8>),
}
```

## Automatic coercion

The dispatcher automatically coerces argument types when safe:

| From → To | Example |
|-----------|---------|
| Int/Float/Bool → String | `42` → `"42"` |
| String → Int | `"42"` → `42` |
| String → Float | `"3.14"` → `3.14` |
| String → Bool | `"true"` → `true`, `"1"` → `true` |
| String → List | `"[1,2,3]"` (JSON) or `"a,b,c"` (CSV) |
| String → Map | `'{"a":1}'` (JSON parse) |

## JSON mapping

Values serialize to JSON naturally:

```json
{
  "null_val": null,
  "bool_val": true,
  "int_val": 42,
  "float_val": 3.14,
  "string_val": "hello",
  "list_val": [1, 2, 3],
  "map_val": {"key": "value"},
  "bytes_val": "deadbeef"
}
```

Bytes are hex-encoded as strings. Handles use a special `{"__handle__": "hdl:..."}` map format.
