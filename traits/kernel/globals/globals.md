# globals

## Purpose

Global statics for trait implementations. Provides `OnceLock`-based singletons set once during bootstrap and accessed by compiled trait implementations at call time. Also provides utility functions for time formatting.

## Exports

* `REGISTRY` — `OnceLock<Registry>`, the loaded trait registry
* `TRAITS_DIR` — `OnceLock<PathBuf>`, path to the traits directory
* `CONFIG` — `OnceLock<Config>`, server configuration
* `START_TIME` — `OnceLock<Instant>`, process start time
* `HANDLES` — `OnceLock<Arc<Mutex<HashMap<String, HandleEntry>>>>`, opaque handle storage
* `HandleEntry` — struct for stored handle metadata
* `now_epoch()` — current time as Unix epoch f64
* `format_uptime(secs)` — format seconds as human-readable duration
* `init(registry, traits_dir, config)` — initialize all globals once

---

## HandleEntry

### Purpose

Metadata for an opaque handle stored in the HANDLES map.

### Fields

* `type_name`: String — type of the handle (e.g. "BrowserContext", "TcpStream")
* `summary`: String — human-readable description
* `created`: f64 — Unix epoch timestamp when created

---

## now_epoch

### Purpose

Get current system time as Unix epoch seconds with sub-second precision.

### Inputs

none

### Outputs

* f64 — seconds since Unix epoch, or 0.0 if system time is before epoch

### State

reads: system clock
writes: none

### Side Effects

none

### Dependencies

* `std::time::SystemTime`

### Flow

1. Get `SystemTime::now()`
2. Compute duration since `UNIX_EPOCH`
3. Convert to `f64` via `as_secs_f64()`
4. Return 0.0 on error

### Edge Cases

* System clock before epoch returns 0.0

### Example

```rust
let t = now_epoch(); // e.g. 1774012800.123
```

---

## format_uptime

### Purpose

Format a duration in seconds as a human-readable string.

### Inputs

* `secs`: f64 — total seconds

### Outputs

* String — formatted as "Xh Ym Zs", "Ym Zs", or "Zs"

### State

reads: none
writes: none

### Side Effects

none

### Dependencies

none

### Flow

1. Cast to u64
2. Compute hours, minutes, seconds
3. If hours > 0, format as "Xh Ym Zs"
4. Else if minutes > 0, format as "Ym Zs"
5. Else format as "Zs"

### Edge Cases

* 0.0 returns "0s"
* Fractional seconds are truncated (e.g. 1.9 -> "1s")
* Negative values: cast to u64 wraps (undefined behavior in practice — not expected)

### Example

```rust
assert_eq!(format_uptime(3661.0), "1h 1m 1s");
assert_eq!(format_uptime(65.0), "1m 5s");
assert_eq!(format_uptime(5.0), "5s");
```

---

## init

### Purpose

Initialize all global statics. Called once during bootstrap.

### Inputs

* `registry`: Registry — loaded trait definitions
* `traits_dir`: PathBuf — path to traits directory
* `config`: Config — server configuration

### Outputs

none

### State

writes: REGISTRY, TRAITS_DIR, CONFIG, START_TIME, HANDLES

### Side Effects

none

### Dependencies

* `Registry`, `Config` from sibling modules
* `tokio::sync::Mutex`, `std::sync::Arc`

### Flow

1. Set REGISTRY to registry
2. Set TRAITS_DIR to traits_dir
3. Set CONFIG to config
4. Set START_TIME to `Instant::now()`
5. Set HANDLES to new empty `Arc<Mutex<HashMap>>`

### Edge Cases

* Second call is silently ignored (OnceLock semantics)
* All `.set()` return values are discarded

### Example

```rust
globals::init(registry, PathBuf::from("./traits"), config);
// Now globals::REGISTRY.get() returns Some(&registry)
```

---

## Internal Structure

All globals are `OnceLock` statics, initialized together via `init()`. `HANDLES` uses `tokio::sync::Mutex` (async-aware) wrapped in `Arc` so it can be shared across async tasks. The other globals are read-only after initialization.

## Notes

* `init()` must be called before any trait implementation tries to access globals
* OnceLock ensures thread-safe one-time initialization — no panics on race
