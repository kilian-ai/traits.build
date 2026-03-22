# traits.build Site & Docs Exploration

## Site Structure

### Top-level site/ directory
- `docusaurus.config.js` — Docusaurus 3.7 config, baseUrl=/traits.build/, dark mode default
- `sidebars.js` — Sidebar structure with docs categories
- `package.json` — Docusaurus v3.7.0, React 18, no custom build
- `.docusaurus/` — Build cache
- `node_modules/` — npm dependencies
- `build/` — Static HTML output from docusaurus build
- `docs/` — 10 markdown source files
- `src/pages/index.js` — Custom homepage (minimal override)
- `src/css/custom.css` — Docusaurus theme customization
- `static/img/` — Contains only .gitkeep (no images)
- `blog/` — Empty directory
- `package-lock.json` — npm lock file

### site/docs/ - 10 markdown files (all full content read)
1. `intro.md` — Introduction to traits.build
2. `getting-started.md` — Prerequisites, build, run, CLI, REST examples
3. `architecture.md` — Directory layout, build system, bootstrap sequence, dispatch flow
4. `trait-definition.md` — `.trait.toml` format guide, sections detail
5. `interfaces.md` — Interface system, requires/provides/bindings, resolution chain, URL-keyed bindings
6. `type-system.md` — TraitType and TraitValue enums, automatic coercion, JSON mapping
7. `rest-api.md` — POST /traits/{namespace}/{name} format, named/positional args, response format, SSE streaming, page routes, OpenAPI spec
8. `cli.md` — traits serve, list, info, call, checksum, test_runner, version, ps, snapshot commands
9. `creating-traits.md` — Step-by-step tutorial for adding new traits, background traits, CLI formatters
10. `deployment.md` — Fly.io setup, Dockerfile (2-stage), fly.toml config, deploy commands, auto-scaling

### site/build/ - Static output (Docusaurus build)
- `index.html` — Main page (Docusaurus v3.9.2, minified, dark theme)
- `404.html` — 404 page
- `docs.html` — Docs landing page
- `sitemap.xml` — SEO sitemap
- `docs/` — 9 HTML files (one per markdown source, except intro.md which merges into docs landing)
  - architecture.html, cli.html, creating-traits.html, deployment.html, getting-started.html, interfaces.html, rest-api.html, trait-definition.html, type-system.html
- `assets/css/` — 1 minified CSS file (styles.f95b0731.css)
- `assets/js/` — 21 JS files (Docusaurus bundled + React + vendors)
- `img/` — May contain logo/favicon referenced in build

## docs/ (Top-level) - 2 files
1. `deploy.md` — Deployment steps to Fly.io (docker buildx, fly deploy, machine management)
2. `release.md` — GitHub release workflow, YYMMDD versioning, tag management

## Current WWW Traits (Page Routing)

### 1. www.traits.build (Landing page)
- File: `traits/www/traits/build/build.rs`
- Entry point: `website()` function
- Provides: `www/webpage` interface
- HTML: Large embedded HTML string with dark theme, hero section, feature cards, architecture diagram, stats
- Serves at: `/` and `/copy` (routes in serve.trait.toml)
- Size: ~15 KB of HTML (inlined in Rust)
- Features:
  - Hero with title, tagline, CTA buttons
  - Stats (traits, namespaces, binary size, 0 deps)
  - Architecture section with 6 arch-boxes
  - Features cards (8x) with icons and descriptions
  - AI-ready section
  - Footer with links

### 2. www.admin (Admin dashboard)
- File: `traits/www/admin/admin.rs`
- Entry point: `admin()` function
- Provides: `www/webpage` interface
- Auth: basic HTTP authentication
- Serves at: `/admin`
- HTML: Complex embedded dashboard (~10 KB) with JavaScript functionality
- Features:
  - Server status (traits count, namespaces, uptime, version)
  - Fly.io machine control (restart, stop/start, destroy)
  - System tools (list traits, run tests, reload registry, version, processes)
  - Fast Deploy button (build + deploy with sftp)
  - Fly.io deploy process documentation
  - Activity logs
- JS functions: callTrait(), checkStatus(), deploy(), scale(), destroy(), listTraits(), runTests(), etc.

### 3. www.docs.api (Redoc API documentation)
- File: `traits/www/docs/api/api.rs`
- Entry point: `api_docs()` function
- Provides: `www/webpage` interface
- Serves at: `/docs/api`
- HTML: Loader page (~3 KB) that fetches OpenAPI spec dynamically
- Feature:
  - Loads Redoc library from CDN
  - Calls `/traits/sys/openapi` endpoint to get OpenAPI spec
  - Renders generated API documentation with dark theme

## Routing System (serve.rs)

### HTTP Server Architecture
- Framework: actix-web
- Port: configurable via TRAITS_PORT
- CORS: Allow all origins, methods, headers

### Routes
```
GET  /health               → health_check() — server status, uptime, trait/namespace count
GET  /metrics              → metrics() — Prometheus format trait count
GET  /traits               → list_traits() — JSON tree of all traits
GET  /traits/{path}        → get_trait_info() — GET trait metadata
POST /traits/{path}        → call_trait() — Call a trait (supports ?stream=1 for SSE)
GET  /* (default)          → serve_page() — Resolve page route via keyed binding
```

### Page Route Resolution
- `serve_page()` resolves URL path to trait via `dispatcher.resolve_keyed(url_path, "kernel.serve")`
- Looks up in `kernel.serve`'s `[bindings]` section
- For `/admin`, checks HTTP Basic Auth before calling trait
- Returns HTML from trait, or 404 if no binding

### Key Mappings (from serve.trait.toml)
```toml
[requires]
"/" = "www/webpage"
"/copy" = "www/webpage"
"/admin" = "www/webpage"
"/docs/api" = "www/webpage"

[bindings]
"/" = "www.traits.build"
"/copy" = "www.traits.build"
"/admin" = "www.admin"
"/docs/api" = "www.docs.api"
```

## Static File Serving

**Current approach: NONE — all content is Rust-only**

- No CSS files served from disk
- No JS files served as separate resources
- No images served (except favicon hardcoded in HTML)
- All pages return inline HTML from traits
- All styling is inline `<style>` blocks
- Page traits use `const HTML: &str = r##"..."##` for embedded markup
