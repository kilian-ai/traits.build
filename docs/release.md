## Release to GitHub

Repo: https://github.com/kilian-ai/traits.build

### Versioning

traits.build uses YYMMDD versioning (e.g. `v260322`). The version is computed automatically by `build.rs` from the build date. If multiple builds happen on the same day, it appends HHMMSS (e.g. `v260322.140530`).

The canonical version lives in `traits/sys/version/version.trait.toml` and is updated by `build.rs` on each build.

**Release goal:** each release should use the clean `vYYMMDD` tag (no intraday suffix). To get this, do the release build as the first build of the day — shortly after midnight UTC. That way `build.rs` sees a new date and produces `vYYMMDD` without appending `.HHMMSS`.

### Steps

```sh
cd path/to/traits.build

# 1. Get the current version
grep 'version =' traits/sys/version/version.trait.toml
# e.g. version = "v260322"

# 2. Commit any pending changes
git add -A && git commit -m "description of changes"

# 3. Push to GitHub
git push origin main

# 4. Tag with the current version
git tag v260322
git push origin v260322

# 5. Create the GitHub release
gh release create v260322 --title "v260322" --notes-file /tmp/release-notes.md
```

### Release notes

Write release notes to a temp file first (avoids shell escaping issues with backticks in markdown):

```sh
cat > /tmp/release-notes.md << 'EOF'
Summary of changes...
EOF
```

Then pass `--notes-file /tmp/release-notes.md` to `gh release create`.

### Updating an existing release

If the tag needs to move to a newer commit:

```sh
# Delete old tag locally and remotely
git tag -d v260322
git push origin :refs/tags/v260322

# Delete the GitHub release
gh release delete v260322 --yes

# Re-tag and re-release
git tag v260322
git push origin v260322
gh release create v260322 --title "v260322" --notes-file /tmp/release-notes.md
```

### Prerequisites

- `gh` CLI authenticated: `gh auth status`
- Remote configured: `git remote add origin https://github.com/kilian-ai/traits.build.git`
