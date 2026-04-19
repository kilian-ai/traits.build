# Apptron Upstream Snapshot

This directory tracks upstream Apptron artifacts vendored by traits.build.

Source:
- Repository: https://github.com/tractordev/apptron
- Runtime JS: assets/wanix.min.js from upstream apptron repository
- Runtime WASM: downloaded from https://apptron.dev/wanix.wasm
- System bundle: downloaded from https://apptron.dev/bundles/sys.tar.gz

Vendored local hosting paths:
- traits/www/static/apptron/index.html
- traits/www/static/apptron/wanix.min.js
- traits/www/static/apptron/wanix.wasm
- traits/www/static/apptron/bundles/sys.tar.gz
- traits/www/static/apptron/wasi/worker/lib.js

Purpose:
- Run Apptron shell under traits.build infrastructure from local static assets.
