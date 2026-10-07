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
| homecache/{uv,node,inventory} | native locked GC and separate shallow inventory |
| maintenance | bounded private success/cadence ledger; no file-age eviction |
| doctor | read-only diagnostic aggregation, actual reports and timestamps |
| system_policy + os/*_policy | native OS policy observation; explicit root APT policy only on Linux |
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
  Currently audited uv 0.12.17/0.12.23, explicit validated cache dir, offline,
  native lock. Prune removes cached environments too: off by default, separate
  uv_prune_rebuildable_environments opt-in required after dependency review.
- Never infer cached environment/linked package preservation from a native
  lock: it coordinates active uv commands, not all future direct Python uses.
- Native npm/pnpm commands without a verified shared install/GC lease remain
  inventory-only. Never launch Corepack shims during discovery/preview.
- Successful GC is throttled by provider/cache identity. Failure never advances
  it; malformed/redirected state blocks GC, preview never writes the ledger.
- APT weekly autoclean is an explicit root CLI operation, never an installer
  side effect. Publish a fixed no-replace policy only, preserving local edits;
  no immediate clean/autoremove, system restart, new root daemon or sudo grant.
- No arbitrary file/tree eviction by mtime, no process-liveness inference,
  no whole-cache wipe, no force/prune-ci, no fallback after native failure.
- Project roots, dependency trees, installed runtimes, Trash, custom paths and
  legacy pending dirs are never deleted by run. Default policy preserves all
  cached environments too; explicit uv prune opt-in has documented exceptions.
- Legacy configs/categories/pressure cannot bypass those boundaries.
- Preview is read-only: it may query tools but never prune, create cache
  content or overwrite last-run state. Show actual effective policy.
- Doctor is stricter: never invokes GC discovery/shims, starts a job, acquires
  the run lock or writes state. Scheduler registration/configuration cannot be
  reported as evidence of completed maintenance. Native snapshots remain OS
  modules; incomplete/unavailable queries are explicit warnings, not success.
- APT cleanup belongs to apt-daily-upgrade install mode, not apt-daily update
  mode. Check effective enable and installed-archive protection; completion
  stamp metadata and wrapper result are distinct evidence. Never force package
  updates just to make a diagnostic green.
- Lock spans a complete actual run. Reports/state stay private; report writes
  are atomic and errors are observable. No backups or historical report copies.
- Paths crossing symlink/reparse-point components are refused for GC/state.
  Native GC is pinned to the exact discovered and validated destination.
- State destinations must be regular bounded files before GC. Status never
  follows redirects, blocks on special files or echoes arbitrary non-report data.
- Truncated native path/version/policy discovery cannot authorize mutation.
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
CI also tests both audited vendor uv binaries on private synthetic caches;
POSIX checks shared-lease refusal/external links, all OSes verify cached-env
removal and default cleaner preservation. Do not use real user cache fixtures.
