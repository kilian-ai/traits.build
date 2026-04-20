---
status: awaiting_human_verify
trigger: "apptron-ide-route-not-loading"
created: 2026-04-19T00:00:00Z
updated: 2026-04-19T00:47:00Z
---

## Current Focus

hypothesis: The SPA ignores hash routes on HTTP, so /#/apptron-ide falls back to / and internal navigation pushes pathname URLs that static hosting cannot resolve reliably.
test: Patch route resolution to prefer location.hash in all environments and push hash URLs for SPA navigation, then rebuild and verify /#/apptron-ide locally.
expecting: The SPA should load Apptron IDE from /#/apptron-ide and preserve hash-based navigation on static hosting.
next_action: user browser-verify /static#/apptron-ide locally and /#/apptron-ide after deployment

## Symptoms

expected: Visiting #/apptron-ide should load the full VS Code/Apptron IDE in the SPA.
actual: User still sees it not coming up / apparent 404 or failure to load. Local browser pages currently show one page titled "traits.build — composable function kernel" on http://127.0.0.1:8092/#/apptron-ide and another with connection-reset history. Server startup attempts often got suspended because traits serve prints REPL output when backgrounded from zsh.
errors: Local browser previously showed net::ERR_CONNECTION_RESET. Terminal logs showed successful startup on 127.0.0.1:8092 with only 17 page routes including /static, but starting the server in background from zsh often yielded "suspended (tty output)". No confirmed browser-console error from the apptron-ide page yet.
reproduction: Open local or deployed SPA at #/apptron-ide. Local dev often uses ./target/release/traits serve -p 8092. Static route checks against /static/apptron-ide/index.html are relevant.
started: This started during the current implementation session after adding the full Apptron IDE route and assets.

## Eliminated

## Evidence

- timestamp: 2026-04-19T00:40:00Z
	checked: traits/www/static/index.html route tables and built static/apptron-ide assets
	found: /apptron-ide is present in ROUTES and STATIC_RUNTIME_ROUTES, and both source and built apptron-ide asset directories exist with index.html, loader.js, and sys.tar.gz reachable.
	implication: The failure is not caused by missing Apptron IDE files or missing /static serving.

- timestamp: 2026-04-19T00:42:00Z
	checked: local HTTP server and direct asset requests
	found: http://127.0.0.1:8092/, /static/apptron-ide/index.html, /static/apptron-ide/vscode/out/vs/loader.js, and /static/apptron/bundles/sys.tar.gz all return 200.
	implication: Local server unreliability reports include TTY/backgrounding noise, but the required Apptron IDE assets are servable when the server is running.

- timestamp: 2026-04-19T00:44:00Z
	checked: currentRouteFromLocation(), navigate(), and hashchange handling in traits/www/static/index.html
	found: currentRouteFromLocation() only reads location.hash when protocol is file:, while HTTP mode reads only location.pathname; navigate() pushes pathname URLs for non-file mode and hashchange listeners are disabled on HTTP.
	implication: Visiting /#/apptron-ide on production or local HTTP will be ignored and fall back to /, matching the reported homepage title instead of Apptron IDE.

- timestamp: 2026-04-19T00:46:00Z
	checked: production root versus rebuilt local SPA shell
	found: https://www.traits.build/ still contains the old non-hash HTTP pushState lines, while http://127.0.0.1:8093/static contains only hash-based pushState lines and still serves /static/apptron-ide/index.html with 200.
	implication: Current production is still unfixed, and the rebuilt local SPA shell has the intended router change.

- timestamp: 2026-04-19T00:46:30Z
	checked: local server entrypoint expectations
	found: The local binary root path / serves the traits.build webpage trait, not the SPA shell; the SPA shell is exposed at /static.
	implication: http://127.0.0.1:8092/#/apptron-ide is a local test misuse even apart from the production hash-routing bug; the correct local SPA URL is http://127.0.0.1:8092/static#/apptron-ide or /static/index.html#/apptron-ide.

## Resolution

root_cause: The SPA router only honors hash routes in file:// mode. On http://127.0.0.1:8092 and https://www.traits.build it ignores /#/apptron-ide during initial route resolution and pushes pathname routes instead of hash routes during navigation.
fix: Prefer hash routes in currentRouteFromLocation() for all environments, always listen for valid hash changes, and push hash-based URLs for SPA navigation while preserving pathname fallback when no hash is present.
verification: 
verification: Rebuilt the SPA, started a fresh no-REPL server on 127.0.0.1:8093, confirmed /static serves the patched router with hash-based pushState and hash-based initial route parsing, confirmed /static/apptron-ide/index.html still returns 200, and confirmed production root still serves the old router logic.
files_changed: ["traits/www/static/index.html", "traits/www/static/index.standalone.html", "static/index.html", "static/index.standalone.html", "index.html"]
