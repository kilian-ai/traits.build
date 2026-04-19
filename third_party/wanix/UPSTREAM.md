# Wanix Upstream Snapshot

This directory tracks the upstream Wanix runtime artifacts used by traits.build.

Source:
- Repository: https://github.com/tractordev/wanix
- Release: v0.3
- Runtime archive: wanix_0.3_runtime.zip
- Shell archive: dep-shell.zip (contains shell.tgz)

Vendored into local static hosting at:
- traits/www/static/wanix/wanix.min.js
- traits/www/static/wanix/wanix.wasm
- traits/www/static/wanix/wasi/worker/lib.js
- traits/www/static/wanix/shell.tgz
- traits/www/static/wanix/index.html

Purpose:
- Run Wanix shell from our own traits.build static assets instead of external wanix.run.
