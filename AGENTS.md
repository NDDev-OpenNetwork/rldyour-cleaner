# rldyour-cleaner — agent instructions

Rust oneshot for native unused-cache GC and optional read-only artifact
inventory. Platforms: Linux, macOS ARM/Intel, Windows. Public repository:
English in code/docs; no host, user, estate paths or private content.

## Layout

| Module | Responsibility |
|---|---|
| cli / lib | argument routing only |
| config | strict TOML, defaults, legacy field compatibility, policy limits |
| runner | one preview/run pipeline and reporting decisions |
| homecache | owning-tool GC adapters and shallow cache inventory |
| process | argv-only native commands, deadlines and bounded pipe draining |
| clean | private per-user run lock; never recursive removal |
| safety | nonredirected dedicated paths, cache marker and protected roots |
| scan / kinds | optional read-only artifact discovery; age is advisory |
| report | atomic private last-run replacement; no archive backups |
| os/mod | compile-time facade + shared environment/disk helpers |
| os/linux, os/macos, os/windows | platform path, process/pipe, file contracts |
| os/unix | POSIX primitives shared by Linux/macOS |
| platforms/{linux,macos,windows} | independent installer/uninstaller + scheduler assets |
| platforms/common | common Unix code staging; no global /tmp override |

## Invariants

- Mutation only through a verified owning-tool unused-entry GC adapter.
  Currently uv >=0.12.17, explicit validated cache dir, offline, native lock.
- No arbitrary file/tree eviction by mtime, no process-liveness inference,
  no whole-cache wipe, no force/prune-ci, no fallback after native failure.
- Project roots, dependency trees, environments, installed runtimes, Trash,
  custom paths and legacy pending dirs are never deleted by run.
- Legacy configs/categories/pressure cannot bypass those boundaries.
- Preview is read-only: it may query tools but never prune, create cache
  content or overwrite last-run state. Show actual effective policy.
- Lock spans a complete actual run. Reports/state stay private; report writes
  are atomic and errors are observable. No backups or historical report copies.
- Paths crossing symlink/reparse-point components are refused for GC/state.
  Native GC is pinned to the exact discovered and validated destination.
- Tool output/latency are bounded. Run only argv, never a shell. Deadlines include readers and owned process trees; Windows spawns suspended
  into a private Job Object before resume. Keep runtime
  dependencies minimal and justified. Never inspect credentials or real user
  data to test; all mutation tests use synthetic cache/private-home fixtures.
- Installer never triggers cleanup, uses no sudo and never changes global
  temporary-file policy. Keep platform scheduler assets and releases aligned.

## Verify

cargo fmt --check; cargo test --locked;
cargo clippy --locked --all-targets -- -D warnings; cargo audit;
shellcheck -x install.sh uninstall.sh platforms/*/*.sh scripts/*.sh; actionlint.

CI performs native tests/Clippy on Linux, macOS ARM/Intel and Windows, MSRV
1.88, cross-target checks, PowerShell/plist/unit checks and synthetic installer
lifecycles (mock schedulers; never create real CI jobs). Release binaries,
matching installers and SHA256 manifests are published only after all five
platform builds succeed. Signed tag must match Cargo/changelog versions.
