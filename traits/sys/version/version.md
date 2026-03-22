# version

## Purpose

Generate trait version strings in YYMMDD date format. Supports date-only and intraday (with HHMMSS suffix) modes. Includes a pure-Rust UTC date calculation (no chrono dependency).

## Exports

* `version(args)` — trait entry point, returns version JSON
* `yymmdd_now()` — current UTC date as "YYMMDD" string (pub, used by snapshot)
* `hhmmss_now()` — current UTC time as "HHMMSS" string (pub, used by snapshot)

---

## utc_now (private)

### Purpose

Compute current UTC date and time components from system clock without chrono.

### Inputs

none

### Outputs

* Tuple `(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32)`

### State

reads: system clock
writes: none

### Side Effects

none

### Dependencies

* `std::time::{SystemTime, UNIX_EPOCH}`

### Flow

1. Get duration since UNIX_EPOCH (default 0 on error)
2. Compute days and time-of-day from total seconds
3. Extract hours, minutes, seconds from time-of-day
4. Apply Howard Hinnant's civil_from_days algorithm to compute year, month, day
5. Adjust month and year for the March-based calendar offset

### Edge Cases

* System time before epoch: uses Duration::default (all zeros → 1970-01-01 00:00:00)

---

## yymmdd_now

### Purpose

Get current UTC date as a 6-digit YYMMDD string.

### Inputs

none

### Outputs

* String — e.g. "260319" for 2026-03-19

### State

reads: system clock
writes: none

### Side Effects

none

### Dependencies

* `utc_now()`

### Flow

1. Call utc_now()
2. Format as `{year%100:02}{month:02}{day:02}`

### Example

```rust
yymmdd_now() // "260319" on 2026-03-19
```

---

## hhmmss_now

### Purpose

Get current UTC time as a 6-digit HHMMSS string.

### Inputs

none

### Outputs

* String — e.g. "143025" for 14:30:25 UTC

### State

reads: system clock
writes: none

### Side Effects

none

### Dependencies

* `utc_now()`

### Flow

1. Call utc_now()
2. Format as `{hour:02}{minute:02}{second:02}`

---

## build_date_version (private)

### Purpose

Build a date-only version JSON object.

### Inputs

none

### Outputs

* JSON: `{"version": "YYMMDD", "date": "YYMMDD", "mode": "date"}`

### Dependencies

* `yymmdd_now()`

---

## build_intraday_version (private)

### Purpose

Build an intraday version JSON object with time suffix.

### Inputs

none

### Outputs

* JSON: `{"version": "YYMMDD.HHMMSS", "date": "YYMMDD", "suffix": "HHMMSS", "mode": "hhmmss"}`

### Dependencies

* `yymmdd_now()`, `hhmmss_now()`

---

## version

### Purpose

Trait entry point. Returns version info based on action.

### Inputs

* `args[0]`: action string — "date" or "hhmmss" (default: "date")

### Outputs

* JSON version object (date or intraday format)

### State

reads: system clock (via helpers)
writes: none

### Side Effects

none

### Dependencies

* `build_date_version`, `build_intraday_version`

### Flow

1. Extract action from first arg, default "date", lowercase trimmed
2. Match:
   - "date" → `build_date_version()`
   - "hhmmss" → `build_intraday_version()`
   - anything else → `build_date_version()` (fallback)

### Edge Cases

* Unknown action falls back to date mode (no error)
* Action is case-insensitive and trimmed

### Example

```
sys.version
=> {"version":"260319","date":"260319","mode":"date"}

sys.version "hhmmss"
=> {"version":"260319.143025","date":"260319","suffix":"143025","mode":"hhmmss"}
```

---

## Internal Structure

`utc_now()` is the core time computation. `yymmdd_now()` and `hhmmss_now()` are public helpers used both internally and by `sys.snapshot`. The version trait dispatches between two JSON builders.

## Notes

* Uses Howard Hinnant's civil_from_days algorithm for calendar computation
* No external time library dependency
* `yymmdd_now` and `hhmmss_now` are public for cross-trait use (snapshot)
