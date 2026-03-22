# checksum

## Purpose

Compute deterministic SHA-256 checksums for JSON values, I/O example pairs, trait signatures, and release update objects. All hashing uses canonical JSON (sorted keys, recursive) for deterministic output.

## Exports

* `canonicalize(value)` — recursively sort object keys for deterministic JSON
* `hash_stable(value)` — SHA-256 of canonicalized JSON value
* `hash_hex(input)` — SHA-256 of raw string
* `hash_bytes(input)` — SHA-256 of raw bytes
* `checksum_for_update(update)` — checksum of an update object excluding its "checksum" field
* `feature_io_pairs(feature)` — extract input/output pairs from a feature's examples
* `io_checksum(features)` — checksum over feature names, I/O pairs, and vtest assertions
* `signature_checksum(toml_text)` — checksum over trait signature params parsed from TOML text
* `checksum_dispatch(args)` — standard dispatch wrapper for uniform trait interface
* `checksum(action, data)` — trait entry point: routes action to hash/io/signature/update

---

## canonicalize

### Purpose

Recursively sort all object keys in a JSON value to produce a canonical form for deterministic serialization.

### Inputs

* value: any serde_json::Value

### Outputs

* A new Value with all object keys sorted alphabetically, arrays recursively canonicalized, primitives unchanged

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* serde_json

### Flow

1. Match on value type
2. If Object: collect keys, sort them, recursively canonicalize each value, rebuild map in sorted order
3. If Array: recursively canonicalize each element
4. Otherwise: clone the value as-is

### Edge Cases

* Empty objects and arrays pass through unchanged
* Nested objects are sorted at every level

### Example

```
canonicalize({"b": 2, "a": {"d": 4, "c": 3}})
→ {"a": {"c": 3, "d": 4}, "b": 2}
```

---

## hash_stable

### Purpose

Compute SHA-256 hex digest of a canonicalized JSON value.

### Inputs

* value: any serde_json::Value

### Outputs

* 64-character lowercase hex string (SHA-256 digest)

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* canonicalize (internal)
* serde_json::to_string
* sha2::Sha256

### Flow

1. Canonicalize the value (sort keys recursively)
2. Serialize to compact JSON string
3. Hash the JSON bytes with SHA-256
4. Format as lowercase hex

### Edge Cases

* If serialization fails, hashes empty string

### Example

```
hash_stable("hello") → "5aa762ae383fbb727af3c7a36d4940a5b8c40a989452d2304fc958ff3f354e7a"
```

---

## hash_hex

### Purpose

SHA-256 hash of a raw string (not JSON-encoded).

### Inputs

* input: string slice

### Outputs

* 64-character lowercase hex string

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* sha2::Sha256

### Flow

1. Hash the input bytes directly with SHA-256
2. Format as lowercase hex

### Edge Cases

* Empty string produces the SHA-256 of empty input

### Example

```
hash_hex("hello") → "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
```

---

## hash_bytes

### Purpose

SHA-256 hash of raw bytes.

### Inputs

* input: byte slice

### Outputs

* 64-character lowercase hex string

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* sha2::Sha256

### Flow

1. Hash the byte slice with SHA-256
2. Format as lowercase hex

### Edge Cases

* Empty byte slice produces SHA-256 of empty input

### Example

```
hash_bytes(b"hello") → "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
```

---

## checksum_for_update

### Purpose

Compute checksum for a release update object, excluding the "checksum" field itself to avoid circular dependency.

### Inputs

* update: serde_json::Value (expected to be an Object)

### Outputs

* 64-character lowercase hex string

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* hash_stable (internal)

### Flow

1. Clone the update value
2. If it's an Object, remove the "checksum" key
3. Hash the remaining object via hash_stable

### Edge Cases

* If update has no "checksum" key, hashes it as-is
* If update is not an Object, hashes whatever it is

### Example

```
checksum_for_update({"version": "1.0", "checksum": "old_hash"})
→ hash_stable({"version": "1.0"})
```

---

## feature_io_pairs

### Purpose

Extract input/output pairs from a feature's examples array for checksum computation.

### Inputs

* feature: serde_json::Value (object with optional "examples" array)

### Outputs

* Vec of objects with "input" and "output" fields extracted from each example

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* none

### Flow

1. Get "examples" from the feature object as an array
2. If empty or missing, return empty vec
3. For each example, extract "input" and "output" (default to null if missing)
4. Build {input, output} objects

### Edge Cases

* Missing "examples" key returns empty vec
* Empty examples array returns empty vec
* Missing input/output in an example defaults to null

### Example

```
feature_io_pairs({"examples": [{"input": ["a"], "output": {"contains": ["x"]}}]})
→ [{"input": ["a"], "output": {"contains": ["x"]}}]
```

---

## io_checksum

### Purpose

Compute a deterministic checksum over feature names, their I/O pairs, and vtest assertions. Used to detect when a trait's behavioral contract changes.

### Inputs

* features: slice of serde_json::Value (array of feature objects)

### Outputs

* 64-character lowercase hex string

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* feature_io_pairs (internal)
* hash_stable (internal)

### Flow

1. Filter features to those that are objects with non-empty "name" string
2. For each feature, extract name, I/O pairs via feature_io_pairs, and vtest assertions
3. Vtest assertions: from "vtests" array, extract each "assert" string, trim, filter empty
4. Skip features where both I/O pairs and assertions are empty
5. Build entry: {name, io, vtest_asserts (if non-empty)}
6. Sort entries by name
7. Hash the sorted array via hash_stable

### Edge Cases

* Features without names are skipped
* Features with empty name (whitespace only) are skipped
* Features with no examples and no vtests are skipped
* Sorting by name ensures deterministic ordering regardless of input order

### Example

```
io_checksum([{"name": "add", "examples": [{"input": [1,2], "output": {"contains": ["3"]}}]}])
→ deterministic hex hash
```

---

## signature_checksum

### Purpose

Compute a deterministic checksum over a trait's signature parameters and return type, parsed from TOML text.

### Inputs

* toml_text: string containing .trait.toml content

### Outputs

* 64-character lowercase hex string

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* regex crate
* hash_stable (internal)

### Flow

1. Define regex patterns for name, type, and optional fields
2. Split TOML text at `[[signature.params]]` headers
3. For each block, extract lines until next section header (line starting with `[`)
4. Parse name (required), type (default "string"), optional (default false)
5. Collect params as {name, type, optional} objects
6. Find `[signature.returns]` section, extract type if present
7. Hash `{params, returns}` via hash_stable

### Edge Cases

* Missing type defaults to "string"
* Missing optional defaults to false
* Blocks without a name are skipped
* No [[signature.params]] sections → empty params array
* No [signature.returns] section → empty returns object

### Example

```
signature_checksum("[[signature.params]]\nname=\"x\"\ntype=\"string\"\n[signature.returns]\ntype=\"object\"")
→ deterministic hex hash
```

---

## checksum_dispatch

### Purpose

Standard dispatch wrapper providing uniform trait interface. Extracts action and data from args array and delegates to `checksum()`.

### Inputs

* args: slice of serde_json::Value — args[0] = action string, args[1] = data value

### Outputs

* serde_json::Value (result from checksum)

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* checksum (internal)

### Flow

1. Extract action from args[0] as string (default empty)
2. Extract data from args[1] (default Null)
3. Call checksum(action, data)

### Edge Cases

* Missing args[0] defaults to empty action string (will hit unknown action error)
* Missing args[1] defaults to Null

---

## checksum

### Purpose

Trait entry point. Routes actions to the appropriate checksum function.

### Inputs

* action: string — "hash", "io", "signature", or "update"
* data: serde_json::Value — depends on action

### Outputs

* Object: {ok: true, checksum: hex_string} on success
* Object: {error: message} on failure

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* hash_stable, io_checksum, signature_checksum, checksum_for_update (internal)

### Flow

1. Match action:
   - "hash": call hash_stable(data), return {ok, checksum}
   - "io": extract data as array, call io_checksum on the slice, return {ok, checksum}
   - "signature": require data to be a string, call signature_checksum on it, return {ok, checksum}; return error if data is not a string
   - "update": require data to be an object, call checksum_for_update, return {ok, checksum}; return error if data is not an object
   - default: return error listing valid actions

### Edge Cases

* "io" with non-array data → treats as empty slice
* "signature" with non-string data → error
* "update" with non-object data → error
* Unknown action → error with list of valid actions

### Example

```
checksum("hash", "hello") → {"ok": true, "checksum": "5aa762ae383fbb727af3c7a36d4940a5b8c40a989452d2304fc958ff3f354e7a"}
checksum("unknown", null) → {"error": "Unknown action: unknown. Use hash, io, signature, or update"}
```

---

## Internal Structure

`checksum_dispatch` is the registered trait entry point — it unpacks args and delegates to `checksum`. `checksum` routes to one of four functions based on action string. All hash operations converge through `hash_stable` which canonicalizes then SHA-256 hashes. `io_checksum` and `signature_checksum` build structured intermediate values before hashing. `checksum_for_update` strips the old checksum field before hashing.

## Notes

* `hash_hex` and `hash_bytes` are marked `#[allow(dead_code)]` — they are public utilities but not called from the trait dispatch path
* `hash_stable` hashes the JSON serialization of the canonicalized value, not the raw input — so `hash_stable("hello")` hashes `"\"hello\""` (quoted JSON string), producing a different result than `hash_hex("hello")` which hashes the raw bytes
