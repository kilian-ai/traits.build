# test_runner

## Purpose

Discover `.features.json` files for traits matching a pattern, run example-based tests (via internal dispatch) and shell command tests, and report structured pass/fail results. Supports both trait-registry and filesystem discovery modes.

## Exports

* `test_runner(args)` — entry point with recursive-call guard
* `test_runner_inner(args)` — core logic: discover, run, aggregate results
* `discover_traits(pattern)` — find traits with features.json via registry lookup
* `discover_fs_features(pattern)` — find features.json from filesystem paths/globs
* `collect_features_recursive(dir, glob_part, out)` — recursive directory walker for features.json
* `load_features(path)` — parse features array from a .features.json file
* `run_example_tests(trait_path, features, params, verbose)` — run examples via internal dispatch
* `run_command_tests(features, verbose)` — run shell command tests
* `input_to_args(input, params)` — convert example input to positional args
* `run_checks(expected, output_str, output, error)` — evaluate contains/not_contains checks
* `deep_equals(a, b)` — value equality
* `object_contains(needle, haystack)` — partial object match (needle keys subset of haystack)
* `object_needle_match(needle, output)` — match needle against output or array items
* `regex_match(pattern, text)` — simple pattern matching without regex crate
* `regex_find_all(pattern, text)` — extract quoted strings from contains patterns
* `parse_count_check(text)` — parse "count >= N" style assertions

---

## test_runner

### Purpose

Entry point with recursive-call prevention. Prevents infinite loops when the test runner's own features.json is discovered and would re-invoke itself.

### Inputs

* args: slice of Value — args[0] = pattern string, args[1] = verbose boolean (optional)

### Outputs

* Object: {ok, pattern, summary, results} on success
* Object: {ok: true, skipped: true, reason} if recursive call detected

### State

reads:
* RUNNING: static AtomicBool

writes:
* RUNNING: set to true on entry, false on exit

### Side Effects

* none (delegates to test_runner_inner)

### Dependencies

* test_runner_inner (internal)

### Flow

1. Attempt to swap RUNNING from false to true
2. If already true (recursive call), return skipped result immediately
3. Call test_runner_inner(args)
4. Reset RUNNING to false
5. Return result

### Edge Cases

* Recursive calls return immediately with {ok: true, skipped: true}
* RUNNING is always reset even if inner function panics (? — no explicit guard)

---

## test_runner_inner

### Purpose

Core test runner logic: discover traits, run tests, aggregate results.

### Inputs

* args: slice of Value — args[0] = pattern string (default "*"), args[1] = verbose boolean (default false)

### Outputs

* Object with: ok, pattern, summary ({traits, examples, commands, skipped, total_passed, total_failed}), results array

### State

reads:
* none

writes:
* none

### Side Effects

* Executes shell commands (via run_command_tests)
* Calls trait dispatch (via run_example_tests)

### Dependencies

* discover_traits, discover_fs_features, load_features, run_example_tests, run_command_tests (internal)

### Flow

1. Extract pattern from args[0] (default "*"), trim whitespace
2. Extract verbose from args[1] (default false)
3. Route discovery: if pattern contains '/' or starts with '.', use discover_fs_features; otherwise discover_traits
4. If no traits found, return error
5. For each trait: load features.json, skip if empty
6. Run example tests (internal dispatch) and command tests (shell)
7. Count passed/failed for both types
8. Build per-trait result with ok, examples, commands counts
9. In verbose mode, include all details; in non-verbose, include only failures
10. Aggregate totals and return summary

### Edge Cases

* Empty pattern defaults to "*"
* Traits with no features or no tests/examples increment skipped counter
* ok is true only when total_failed == 0

---

## discover_traits

### Purpose

Find traits with .features.json files by matching against the registry using namespace.name patterns.

### Inputs

* pattern: string — e.g., "sys.checksum", "sys.*", "*"

### Outputs

* Vec of (trait_path, features_json_path, params) tuples

### State

reads:
* crate::globals::REGISTRY

writes:
* none

### Side Effects

* none

### Dependencies

* crate::globals::REGISTRY

### Flow

1. Split pattern at '.' into namespace filter and name filter
2. If no '.', namespace is empty, name is the pattern
3. Iterate all registry entries
4. Split each entry.path into (ns, name)
5. Match against filters: "*" matches all, empty ns_filter matches all namespaces
6. For matching traits, look for `{name}.features.json` in the same directory as the .trait.toml
7. If features file exists, collect trait path, features path, and param list
8. Sort results by trait path

### Edge Cases

* Pattern "*" matches all traits
* Pattern "sys.*" matches all in sys namespace
* Traits without features.json are silently skipped
* Entry paths that don't contain '.' are skipped

---

## discover_fs_features

### Purpose

Find .features.json files from filesystem paths, directory listings, or glob patterns.

### Inputs

* pattern: string — e.g., "./src/cli.features.json", "./src/", "./traits/sys/*"

### Outputs

* Vec of (display_label, features_json_path, empty_params) tuples

### State

reads:
* filesystem

writes:
* none

### Side Effects

* none

### Dependencies

* std::fs
* collect_features_recursive (internal)

### Flow

1. Three sub-cases based on path type:
   - **Direct file**: if pattern is an existing file ending in ".features.json", use it
   - **Directory**: if pattern is a directory, list all *.features.json files in it (non-recursive, one level)
   - **Glob**: extract parent directory and filename pattern, recursively walk with collect_features_recursive
2. Sort collected files
3. Build display labels: dir_name/file_stem (stripping ".features" suffix)
4. Return with empty params (no trait signature available for filesystem discovery)

### Edge Cases

* Filesystem discovery returns empty params → example tests relying on param mapping won't work
* Non-existent paths yield no results
* Display label uses parent directory name + base name

---

## collect_features_recursive

### Purpose

Recursively walk a directory tree collecting .features.json files matching a glob pattern.

### Inputs

* dir: path to search
* glob_part: filename pattern (e.g., "*", "cli*")
* out: accumulator vec for found paths

### Outputs

* none (appends to out)

### State

reads:
* filesystem

writes:
* out (accumulator)

### Side Effects

* filesystem traversal

### Dependencies

* std::fs

### Flow

1. Read directory entries
2. For each entry:
   - If directory: recurse into it
   - If file ending in ".features.json": check glob match
3. Glob matching: strip trailing '*' from glob_part to get prefix
4. If prefix is empty (glob was "*"), accept all
5. Otherwise, filename must start with prefix

### Edge Cases

* Unreadable directories are silently skipped
* "*" matches all features.json files
* "cli*" matches "cli.features.json"

---

## load_features

### Purpose

Load and parse the "features" array from a .features.json file.

### Inputs

* path: string path to .features.json file

### Outputs

* Option of Vec of Value — None if file can't be read or parsed

### State

reads:
* filesystem

writes:
* none

### Side Effects

* none

### Dependencies

* std::fs, serde_json

### Flow

1. Read file to string
2. Parse as JSON
3. Extract "features" key as array
4. Return cloned array or None

### Edge Cases

* File not found → None
* Invalid JSON → None
* Missing "features" key → None

---

## run_example_tests

### Purpose

Run example-based tests by calling traits via internal dispatch and checking output against expected contains/not_contains patterns.

### Inputs

* trait_path: string (e.g., "sys.checksum")
* features: slice of feature Value objects
* params: slice of (name, type) tuples for the trait signature
* verbose: boolean

### Outputs

* Vec of test result Value objects

### State

reads:
* none

writes:
* none

### Side Effects

* Calls traits via crate::dispatcher::compiled::dispatch

### Dependencies

* crate::dispatcher::compiled::dispatch
* input_to_args, run_checks (internal)

### Flow

1. For each feature, get feature name
2. For each example in feature's "examples" array:
   - Convert input to args using input_to_args with trait params
   - Call trait via internal dispatch
   - Get output (or error if dispatch returns error/None)
   - Serialize output to compact JSON string
   - Run checks from example's "output" expected value
   - Determine passed: if no checks, passed means no error; if checks exist, all must be ok
3. Build result entry with type, feature name, passed status
4. In verbose: include input, output, checks
5. In non-verbose failure: include input and failing checks only

### Edge Cases

* Trait not found in compiled dispatch → error
* Example without "input" → empty args
* Example without "output" or empty checks → passed if no error
* Features without "examples" array are skipped

---

## input_to_args

### Purpose

Convert example input (array, object, or scalar) to positional args for trait dispatch.

### Inputs

* input: Option of Value — the example's "input" field
* params: slice of (name, type) tuples

### Outputs

* Vec of Value — positional args

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

1. If Array: pass through directly
2. If Object: map named keys to positional args using param order
   - Find the last param index referenced in the object
   - Fill args array up to that index, looking up each param name
   - Missing params get Null
3. If scalar: wrap in single-element array
4. If None: empty array

### Edge Cases

* Object input with keys not matching any param → those keys are ignored
* Gaps in param mapping filled with Null
* Empty object → empty args

---

## run_checks

### Purpose

Evaluate contains and not_contains assertions against trait output.

### Inputs

* expected: Value — the example's "output" object with "contains" and/or "not_contains" arrays
* output_str: compact JSON string of the output
* output: the raw Value output
* error: optional error string

### Outputs

* Vec of check result objects {type, value, ok}

### State

reads:
* none

writes:
* none

### Side Effects

* none

### Dependencies

* object_needle_match (internal)

### Flow

1. If expected has "contains" array:
   - For each needle: if string, check output_str.contains; if object/other, use object_needle_match
   - Record {type: "contains", value, ok}
2. If expected has "not_contains" array:
   - Same logic but inverted: string must NOT be in output_str, object must NOT match

### Edge Cases

* Non-string needles use deep object matching instead of string containment
* Error responses are wrapped as {ok: false, error} for object matching

---

## run_command_tests

### Purpose

Run shell command tests defined in features and check exit codes, output containment, and count assertions.

### Inputs

* features: slice of feature Value objects
* verbose: boolean

### Outputs

* Vec of test result Value objects

### State

reads:
* crate::globals::TRAITS_DIR (for working directory)

writes:
* none

### Side Effects

* Executes shell commands via `sh -c`

### Dependencies

* std::process::Command
* crate::globals::TRAITS_DIR

### Flow

1. Determine cwd: TRAITS_DIR parent or current directory
2. For each feature, iterate its "tests" array
3. For each test with a "command" string:
   - Execute via `sh -c <command>` in the cwd
   - Capture stdout, stderr, exit code
4. If structured "checks" array exists:
   - Evaluate each check by type: exit_code, contains, not_contains, count_gte
5. If legacy "expect" string exists:
   - Parse for "exit 0", "exit non-zero", "contains 'X'", "count >= N" patterns
6. Build result entry with test name, feature name, passed status
7. Verbose: include command, stdout (truncated to 500), stderr (truncated to 500), exit code, checks
8. Non-verbose failure: include command and fail reasons

### Edge Cases

* Tests without "command" field are skipped
* Command execution failure → exit code 1, stderr has error message
* stdout/stderr truncated to 500 chars in verbose output
* Both stdout and stderr are checked for "contains" assertions
* "count_gte" parses stdout as integer

---

## deep_equals

### Purpose

Value equality check (delegates to PartialEq).

### Inputs

* a, b: Value references

### Outputs

* boolean

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* none

### Flow

1. Return a == b

### Edge Cases

* none

---

## object_contains

### Purpose

Check if all keys in needle exist in haystack with equal values (partial object match).

### Inputs

* needle, haystack: Value references

### Outputs

* boolean

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* deep_equals (internal)

### Flow

1. If both are objects: check every key in needle exists in haystack with matching value
2. Otherwise: use deep_equals

### Edge Cases

* Empty needle matches any object
* Non-object values fall back to deep equality

---

## object_needle_match

### Purpose

Match a needle against output — if output is an array, check any element; otherwise check directly.

### Inputs

* needle, output: Value references

### Outputs

* boolean

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* object_contains (internal)

### Flow

1. If output is Array: return true if any element matches via object_contains
2. Otherwise: check directly via object_contains

### Edge Cases

* Empty array → false (no match)

---

## regex_match

### Purpose

Simple pattern matching for legacy expect strings without pulling in the regex crate.

### Inputs

* pattern: string literal identifying the check type
* text: string to search

### Outputs

* boolean

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* none

### Flow

1. Match pattern against known patterns:
   - `exits?\s+0\b` → check for "exit 0" or "exits 0"
   - `non.?zero` → check for "non-zero", "nonzero", "non zero"
   - `exits?\s+non.?zero` → check for "exit non-zero" variants
   - default: plain substring containment

### Edge Cases

* Only handles specific known patterns, not arbitrary regex

---

## regex_find_all

### Purpose

Extract quoted strings from "contains 'X'" patterns in legacy expect strings.

### Inputs

* pattern: string (only handles "contains" patterns)
* text: expect string

### Outputs

* Vec of extracted strings

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* none

### Flow

1. If pattern contains "contains":
   - Search for "contains " and "contain " prefixes
   - After each prefix, skip whitespace, find opening quote (' or ")
   - Extract content until closing quote
2. Otherwise: return empty vec

### Edge Cases

* Supports both single and double quotes
* Multiple contains clauses are all extracted

---

## parse_count_check

### Purpose

Parse "count >= N" style assertions from legacy expect strings.

### Inputs

* text: expect string

### Outputs

* Option of (operator, threshold) — e.g., (">=", 5)

### State

reads: none
writes: none

### Side Effects

* none

### Dependencies

* none

### Flow

1. Find "count" in the lowercase text
2. After "count", try matching operators: >=, <=, ==, >, <, =
3. Parse the number after the operator
4. Return (operator, number)

### Edge Cases

* "count" not found → None
* No valid operator after "count" → None
* Non-numeric value after operator → None

---

## Internal Structure

`test_runner` wraps `test_runner_inner` with an AtomicBool guard to prevent recursive invocation. The inner function routes discovery to either `discover_traits` (registry lookup) or `discover_fs_features` (filesystem walk) based on pattern format. For each discovered features.json, it loads features then runs two test types: `run_example_tests` (internal dispatch) and `run_command_tests` (shell execution). Example tests use `input_to_args` for param mapping and `run_checks` for output validation. Command tests support both structured "checks" arrays and legacy "expect" strings with built-in simple pattern matchers.

## Notes

* `discover_fs_features` returns empty params, so example tests that rely on Object→positional mapping via params won't work for filesystem-discovered traits — only Array inputs work
* The RUNNING guard uses SeqCst ordering for maximum safety but there's no panic guard (if inner panics, RUNNING stays true)
* stdout/stderr in verbose command output are truncated to 500 chars
* The regex helpers are hand-rolled string matching, not actual regex — only specific known patterns are supported
