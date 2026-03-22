# config.rs

## Purpose

Server configuration loading and defaults. Reads `traits.toml`, applies environment variable overrides, and provides sensible defaults when the config file is absent.

## Exports

* `Config` — top-level configuration struct (traits + workers)
* `TraitsConfig` — core settings: traits directory, port, timeout, bindings file
* `WorkerConfig` — per-language worker process settings

---

## Config

### Purpose

Top-level configuration container holding trait settings and optional worker definitions.

### Fields

* traits: TraitsConfig — core server settings
* workers: HashMap\<String, WorkerConfig\> — per-language worker configs (currently unused by kernel)

### Field Usage

* **traits** — actively used: `traits.traits_dir` (cli.rs bootstrap), `traits.port` (main.rs, dispatcher.rs serve), `traits.timeout` (cli.rs bootstrap → Dispatcher::new)
* **workers** — never read at runtime; `#[warn(dead_code)]` suppressed. Parsed from TOML but not consumed by the kernel.

---

## Config::load

### Purpose

Load configuration from a TOML file path, falling back to defaults if the file does not exist.

### Inputs

* path: file path to traits.toml (e.g. "traits.toml")

### Outputs

* Ok(Config) — parsed or default config
* Err — if file exists but fails to parse

### State

reads: filesystem (traits.toml), environment variables
writes: none

### Side Effects

* Reads file from disk
* Reads TRAITS_PORT, TRAITS_DIR, TRAITS_TIMEOUT environment variables

### Dependencies

* std::fs::read_to_string
* toml::from_str
* std::env::var

### Flow

1. Check if path exists on disk
2. If exists: read file, parse TOML into Config
3. Apply environment variable overrides (TRAITS_PORT → port, TRAITS_DIR → traits_dir, TRAITS_TIMEOUT → timeout)
4. If file does not exist: return Config with all defaults
5. Return Ok(Config)

### Edge Cases

* Missing file → defaults returned, no error
* File exists but invalid TOML → error returned
* Environment variable present but unparseable (e.g. TRAITS_PORT="abc") → silently ignored, file/default value kept

### Example

```
// File exists with port = 9090
Config::load("traits.toml") → Config { traits: { port: 9090, ... }, workers: {} }

// No file
Config::load("nonexistent.toml") → Config { traits: { port: 8080, timeout: 30, traits_dir: "./traits" }, workers: {} }

// File has port = 9090, env TRAITS_PORT=3000
Config::load("traits.toml") → Config { traits: { port: 3000, ... }, ... }
```

---

## TraitsConfig

### Purpose

Core trait server settings.

### Fields

* traits_dir: path to traits directory (default: "./traits")
* port: HTTP server port (default: 8080)
* timeout: trait call timeout in seconds (default: 30)
* bindings_file: path to capability bindings file (default: "./state/bindings.cl")

### Field Usage

* **traits_dir** — used by cli.rs bootstrap to locate trait definitions
* **port** — used by main.rs and dispatcher.rs for HTTP server binding
* **timeout** — passed to Dispatcher::new as call timeout
* **bindings_file** — parsed but never read at runtime (bindings DashMap populated externally)

---

## WorkerConfig

### Purpose

Per-language worker process configuration. Parsed from `[workers.<lang>]` TOML sections.

### Fields

* command: executable path (e.g. "python3")
* args: command-line arguments
* env: environment variables for the worker process
* pool_size: number of worker processes to spawn (default: 1)

### Field Usage

All fields are currently unused — the kernel does not spawn external worker processes. The struct exists for forward compatibility with the full platform's worker pool system.

---

## Default Functions (private)

* `default_pool_size` → 1
* `default_traits_dir` → "./traits"
* `default_port` → 8080
* `default_timeout` → 30
* `default_bindings_file` → "./state/bindings.cl"

---

## Internal Structure

Simple two-layer config: `Config` wraps `TraitsConfig` + optional `WorkerConfig` map. Loading is a single function with env-var override pass. No complex initialization or validation.

## Notes

* Environment variables take precedence over file values (override applied after TOML parse)
* Invalid env var values are silently ignored — the file or default value is preserved
* `WorkerConfig` and `Config.workers` are dead code in the kernel; kept for TOML compatibility with the full platform's `traits.toml`
* `bindings_file` in TraitsConfig is parsed but not consumed — bindings are managed by the Registry's DashMap at runtime
