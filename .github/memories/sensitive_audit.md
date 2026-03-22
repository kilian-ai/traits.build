# Sensitive Content Audit - traits.build

## Summary
Found multiple personal account references, deployment configuration, and hardcoded app names that would be problematic in a public crate. Total findings: 50+ instances across multiple file types.

## Cargo.toml Exclude List
```
exclude = [
    "programs/",
    "target/",
    "gui/",
    "scripts/",
    "*.bak",
]
```

## Key Issue
Most .features.json files, fly.toml, Dockerfile, and .github/ ARE NOT excluded, so they WILL be shipped in the published crate.

## Findings Breakdown

### Category 1: GitHub Account References (7+ instances)
- Cargo.toml authors = "kilian"
- Cargo.toml repository = github.com/kilian-ai/traits.build
- Multiple .md files and site files link to kilian-ai/traits.build
- README.md homepage = traits.build

### Category 2: Fly.io Configuration (30+ instances)
- Hard-coded app name: "polygrait-api"
- Hard-coded region: "iad"
- Hard-coded port: 8090
- Docker registry: registry.fly.io/polygrait-api:*

### Category 3: Personal Paths in Test Commands (12 instances)
All in .features.json files - WILL BE SHIPPED:
- /Users/kilian/.ai/traits/Polygrait/AB.\ Traits
- /Users/kilian/.ai/traits/Polygrait/A. traits.build

### Category 4: Admin/Auth References (5 instances)
- ADMIN_PASSWORD environment variable reference
- FLY_API_TOKEN environment variable reference
- CLOUDFLARE_API_TOKEN mentioned in admin.rs
- Basic auth hardcoded username "admin"

### Category 5: Hardcoded Fly.io URLs (4+ instances)
- https://traits.build/health (hardcoded in scripts)
- registry.fly.io references in multiple files
- fly deploy commands hardcoded with app name

### Category 6: Documentation Exposures
- docs/deploy.md: Full deployment instructions with hardcoded app name
- site/docs/deployment.md: Live instance URL and configuration details
- README.md: Full deployment instructions with registry.fly.io/polygrait-api
