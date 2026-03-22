# snapshot

## Purpose

Version-stamp a trait's TOML definition file. Reads the current version, generates a new YYMMDD-based version (appending HHMMSS suffix if already stamped today), and writes it back.

## Exports

* `snapshot(args)` — trait entry point, bumps version in a trait's .toml file

---

## snapshot

### Purpose

Bump the version field in a trait's `.trait.toml` file to today's date-based version.

### Inputs

* `args[0]`: trait_path string (required, e.g. "sys.checksum")

### Outputs

* JSON: `{"ok": true, "trait_path", "old_version", "new_version", "toml_path"}` on success
* JSON: `{"ok": false, "error": "..."}` on failure

### State

reads:
* `crate::globals::REGISTRY` — to look up trait entry and toml_path

writes:
* Filesystem — rewrites the trait's .toml file with updated version

### Side Effects

* Writes to filesystem (modifies .trait.toml file)

### Dependencies

* `crate::globals::REGISTRY`
* `super::version::{yymmdd_now, hhmmss_now}` — for date/time strings
* `std::fs::{read_to_string, write}`

### Flow

1. Extract trait_path from args[0]; error if empty or missing
2. Get REGISTRY; error if not initialized
3. Look up trait entry; error if not found
4. Read toml_path content; error if unreadable
5. Extract current version via `extract_version()`, default "v000000"
6. Compute today's YYMMDD string
7. If old version already starts with today's date → new version = "YYMMDD.HHMMSS"
8. Otherwise → new version = "YYMMDD"
9. Replace version in TOML via `set_version()`
10. Write updated TOML back; error if write fails
11. Return success JSON

### Edge Cases

* Empty trait_path: returns error
* Registry not initialized: returns error
* Trait not found: returns error
* File unreadable: returns error
* File unwritable: returns error
* No version line in TOML: defaults old_version to "v000000"
* Already versioned today: appends HHMMSS suffix for uniqueness

### Example

```
sys.snapshot "sys.checksum"
=> {"ok":true,"trait_path":"sys.checksum","old_version":"v260319","new_version":"v260322","toml_path":"./traits/sys/checksum/checksum.trait.toml"}
```

---

## extract_version (private)

### Purpose

Parse version string from TOML text by finding the first `version = "..."` line.

### Inputs

* `toml`: TOML content string

### Outputs

* `Option<String>` — version value or None

### Flow

1. Scan lines for one starting with "version"
2. Split on `=`, take second part
3. Trim whitespace and quotes
4. Return if non-empty

### Edge Cases

* No version line: returns None
* Empty version value: returns None

---

## set_version (private)

### Purpose

Replace the first `version = "..."` line in TOML text with a new version.

### Inputs

* `toml`: original TOML string
* `new_version`: version string to set

### Outputs

* String — updated TOML content

### Flow

1. Iterate lines
2. First line matching `version` + `=`: replace with `{indent}version = "{new_version}"`
3. Preserve original indentation
4. Only replace once (first occurrence)
5. Trim trailing newline if original didn't have one

### Edge Cases

* Indented version lines: preserves leading whitespace
* Multiple version-like lines: only first is replaced

---

## Internal Structure

`snapshot` orchestrates: registry lookup → file read → version extraction → date computation → version replacement → file write. `extract_version` and `set_version` are pure string-processing helpers.

## Notes

* Version format: "YYMMDD" for first snapshot of the day, "YYMMDD.HHMMSS" for subsequent ones
* Uses `super::version::` to access `yymmdd_now` and `hhmmss_now` from the sibling version trait
