---
name: traits.build
description: |
  Invoke traits.build API functions as MCP tools or REST calls.
  Every trait is available as an MCP tool (dot→underscore naming)
  and as a REST endpoint (POST /traits/{namespace}/{name}).
---

# traits.build Skills

> Auto-generated from OpenAPI spec v260322.215420 — [traits.build](https://traits.build)

## How to call traits

Every trait can be invoked two ways:

### MCP Tool Call

Tool names use underscore notation: `sys.checksum` → `mcp_traits-build_sys_checksum`

```json
{
  "name": "mcp_traits-build_sys_checksum",
  "arguments": { "action": "hash", "data": "hello" }
}
```

### REST API Call

```bash
curl -X POST https://traits.build/traits/sys/checksum \
  -H 'Content-Type: application/json' \
  -d '{"args": ["hash", "hello"]}'
```

---

## Available Traits

### sys — System utilities — registry, checksums, versioning, testing

| MCP Tool | Trait Path | Description |
|----------|------------|-------------|
| `sys_checksum` | `sys.checksum` | Compute deterministic SHA-256 checksums: hash values, I/O pairs, or trait signatures |
| `sys_cli` | `sys.cli` | CLI bootstrap, trait dispatch, stdin injection, arg parsing, result formatting |
| `sys_docs_skills` | `sys.docs.skills` | Generate a SKILL.md file from the OpenAPI spec — teaches AI agents to use traits as MCP tools |
| `sys_info` | `sys.info` | Show detailed info about a specific trait |
| `sys_list` | `sys.list` | List all registered traits |
| `sys_mcp` | `sys.mcp` | MCP stdio server — exposes all traits as MCP tools via JSON-RPC 2.0 over stdin/stdout |
| `sys_openapi` | `sys.openapi` | Generate OpenAPI 3.0 specification from the trait registry |
| `sys_ps` | `sys.ps` | List running background traits with process details |
| `sys_registry` | `sys.registry` | Registry read API — list, info, query, search, and inspect registered traits |
| `sys_snapshot` | `sys.snapshot` | Snapshot a trait version: sets version in its .trait.toml to YYMMDD, or YYMMDD.HHMMSS if today's date is already the current version |
| `sys_test_runner` | `sys.test.runner` | Generic test runner. Discovers .features.json files for traits matching a glob pattern, runs example-based tests (via internal dispatch) and shell command tests, reports structured pass/fail results. |
| `sys_version` | `sys.version` | Show trait system version, or generate YYMMDD version strings |

#### `sys.checksum`

Compute deterministic SHA-256 checksums: hash values, I/O pairs, or trait signatures

- **MCP tool:** `sys_checksum`
- **REST:** `POST /traits/sys/checksum`

#### `sys.cli`

CLI bootstrap, trait dispatch, stdin injection, arg parsing, result formatting

- **MCP tool:** `sys_cli`
- **REST:** `POST /traits/sys/cli`

#### `sys.docs.skills`

Generate a SKILL.md file from the OpenAPI spec — teaches AI agents to use traits as MCP tools

- **MCP tool:** `sys_docs_skills`
- **REST:** `POST /traits/sys/docs/skills`

#### `sys.info`

Show detailed info about a specific trait

- **MCP tool:** `sys_info`
- **REST:** `POST /traits/sys/info`

#### `sys.list`

List all registered traits

- **MCP tool:** `sys_list`
- **REST:** `POST /traits/sys/list`

#### `sys.mcp`

MCP stdio server — exposes all traits as MCP tools via JSON-RPC 2.0 over stdin/stdout

- **MCP tool:** `sys_mcp`
- **REST:** `POST /traits/sys/mcp`

#### `sys.openapi`

Generate OpenAPI 3.0 specification from the trait registry

- **MCP tool:** `sys_openapi`
- **REST:** `POST /traits/sys/openapi`

#### `sys.ps`

List running background traits with process details

- **MCP tool:** `sys_ps`
- **REST:** `POST /traits/sys/ps`

#### `sys.registry`

Registry read API — list, info, query, search, and inspect registered traits

- **MCP tool:** `sys_registry`
- **REST:** `POST /traits/sys/registry`

#### `sys.snapshot`

Snapshot a trait version: sets version in its .trait.toml to YYMMDD, or YYMMDD.HHMMSS if today's date is already the current version

- **MCP tool:** `sys_snapshot`
- **REST:** `POST /traits/sys/snapshot`

#### `sys.test.runner`

Generic test runner. Discovers .features.json files for traits matching a glob pattern, runs example-based tests (via internal dispatch) and shell command tests, reports structured pass/fail results.

- **MCP tool:** `sys_test_runner`
- **REST:** `POST /traits/sys/test_runner`

#### `sys.version`

Show trait system version, or generate YYMMDD version strings

- **MCP tool:** `sys_version`
- **REST:** `POST /traits/sys/version`

### www — Web interface — landing page, admin dashboard, deployment

| MCP Tool | Trait Path | Description |
|----------|------------|-------------|
| `www_admin` | `www.admin` | Admin dashboard for traits.build deployment |
| `www_admin_deploy` | `www.admin.deploy` | Deploy latest version to Fly.io |
| `www_admin_destroy` | `www.admin.destroy` | Destroy all Fly.io machines for the app |
| `www_admin_fast_deploy` | `www.admin.fast.deploy` | Run fast-deploy.sh locally: build amd64 binary in Docker + upload via sftp + restart. Only works from local dev machine. |
| `www_admin_save_config` | `www.admin.save.config` | Save deploy configuration to traits.toml |
| `www_admin_scale` | `www.admin.scale` | Scale Fly.io machines (0 = stop all, 1+ = start) |
| `www_docs` | `www.docs` | Serve the documentation site — all docs rendered from markdown |
| `www_docs_api` | `www.docs.api` | Serve the API documentation page (Redoc) |
| `www_traits_build` | `www.traits.build` | Landing page for traits.build |

#### `www.admin`

Admin dashboard for traits.build deployment

- **MCP tool:** `www_admin`
- **REST:** `POST /traits/www/admin`

#### `www.admin.deploy`

Deploy latest version to Fly.io

- **MCP tool:** `www_admin_deploy`
- **REST:** `POST /traits/www/admin/deploy`

#### `www.admin.destroy`

Destroy all Fly.io machines for the app

- **MCP tool:** `www_admin_destroy`
- **REST:** `POST /traits/www/admin/destroy`

#### `www.admin.fast.deploy`

Run fast-deploy.sh locally: build amd64 binary in Docker + upload via sftp + restart. Only works from local dev machine.

- **MCP tool:** `www_admin_fast_deploy`
- **REST:** `POST /traits/www/admin/fast_deploy`

#### `www.admin.save.config`

Save deploy configuration to traits.toml

- **MCP tool:** `www_admin_save_config`
- **REST:** `POST /traits/www/admin/save_config`

#### `www.admin.scale`

Scale Fly.io machines (0 = stop all, 1+ = start)

- **MCP tool:** `www_admin_scale`
- **REST:** `POST /traits/www/admin/scale`

#### `www.docs`

Serve the documentation site — all docs rendered from markdown

- **MCP tool:** `www_docs`
- **REST:** `POST /traits/www/docs`

#### `www.docs.api`

Serve the API documentation page (Redoc)

- **MCP tool:** `www_docs_api`
- **REST:** `POST /traits/www/docs/api`

#### `www.traits.build`

Landing page for traits.build

- **MCP tool:** `www_traits_build`
- **REST:** `POST /traits/www/traits/build`

---

*Generated by `sys.docs.skills` from traits.build v260322.215420 — [API Docs](https://traits.build/docs/api)*
