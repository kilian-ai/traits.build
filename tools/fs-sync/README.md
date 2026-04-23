# fs-sync

A tiny standalone 9P2000.L file server in Rust. Exposes a local folder over TCP
so a 9P client (Linux kernel `v9fs`, `9pfuse`, etc.) can mount it live.

Zero external dependencies — just `std`. Builds to a ~300 KB stripped binary.

## Build

```sh
cd tools/fs-sync
cargo build --release
```

Binary: `target/release/fs-sync`.

## Run

```sh
fs-sync --dir ~/projects/my-folder
# fs-sync: serving /Users/you/projects/my-folder on 127.0.0.1:5640 (9P2000.L)
```

Flags:

| flag | default | meaning |
|---|---|---|
| `--dir, -d <path>` | (required) | folder to share (sandboxed — client cannot escape it) |
| `--host, -H <addr>` | `127.0.0.1` | bind address. Stay on loopback unless you add your own auth in front. |
| `--port, -p <port>` | `5640` | TCP port. 564 is the 9P well-known port but needs root. |
| `--readonly, -r` | off | refuse writes, creates, renames, unlinks |

## Mount from a Linux client

```sh
mount -t 9p -o trans=tcp,port=5640,version=9p2000.L,aname=/ \
      <server_ip> /mnt/host
```

Useful extra options: `msize=65536` (bigger reads), `cache=loose` (aggressive
client caching), `dfltuid=0,dfltgid=0` (map owner).

## Supported operations

Protocol: **9P2000.L** (Linux dialect). Implemented:

- version, attach (no auth), walk, clunk, flush
- lopen, lcreate, read, write, readdir
- getattr, setattr (mode + truncate)
- mkdir, rename, renameat, unlinkat, remove
- fsync, statfs

Not implemented (returns `ENOTSUP`): xattr, locks, symlinks, hardlinks, mknod.
Plenty for an editor working folder — `vim`, `git`, `npm install`, etc.

## Security notes

- No authentication. `--host 127.0.0.1` is the default for a reason.
- Paths are sandboxed: `..` traversal that would escape the root is rejected.
- All clients share the same view of the folder.
- Runs as your user; file perms/owners match whatever your shell can create.

## Not part of the traits binary

This lives outside the main cargo workspace on purpose — it's its own crate
with its own `[workspace]` marker. Build and ship independently.
