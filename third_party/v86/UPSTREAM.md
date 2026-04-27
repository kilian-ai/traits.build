# v86 (upstream)

Upstream: https://github.com/copy/v86
Pinned to tag/commit: `latest` (`452df77` — "v86.d.ts refactor #1533")

This directory is a shallow clone used to (re)build `static/v86/libv86.js`
from source so we can patch / debug v86's 9P filesystem code. The clone
itself is git-ignored — only this `UPSTREAM.md` is tracked.

## Rebuild

```sh
# from repo root
git clone --depth=1 https://github.com/copy/v86.git third_party/v86
cd third_party/v86

# closure compiler is downloaded automatically, but on systems without
# wget the `make $(CLOSURE)` target fails — fetch it manually:
mkdir -p closure-compiler
curl -sSL -o closure-compiler/compiler.jar \
  https://repo1.maven.org/maven2/com/google/javascript/closure-compiler/v20210601/closure-compiler-v20210601.jar

make build/libv86.js
cp build/libv86.js ../../static/v86/libv86.js
cp build/libv86.js ../../traits/www/static/v86/libv86.js
```

Requires: `java` (any JRE 11+), `make`. Build takes ~12 s.

## Debugging the 9P stack

The relevant source files live in `lib/`:

- `lib/9p.js` — wire protocol, op dispatch (`Tgetattr=24`, `Tunlinkat=76`, `Tlcreate=14`, ...)
- `lib/filesystem.js` — inode model (`uc` constructor / `nlinks`, `link_under_dir`, `Unlink`, `CreateFile`)
- `lib/marshall.js` — wire encoder/decoder. Note: type `"d"` (qword) only writes the low 32 bits and pads the high 4 bytes with zeros.

To produce a debug-readable build replace `build/libv86.js` with `build/libv86-debug.js` (`make build/libv86-debug.js`).
