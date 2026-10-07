# rldyour-cleaner

A small Rust oneshot that schedules **native unused-cache garbage collection**
and reports other developer caches. Linux, macOS and Windows use their OS
scheduler; there is no resident daemon or continuous polling.

Version 0.1 removes the 0.0.x age-based deletion paths. Old timestamps, missing
open file handles, a `CACHEDIR.TAG`, and a successful directory rename do **not**
prove that files are unused. This matters for installed tools, virtual
environments, package stores, build outputs and lazily loaded application data.

## What runs automatically

| Area | Decision |
|---|---|
| uv >=0.12.17 | `uv cache prune`: the owner identifies dangling/unused entries and coordinates in-use checks through its lock |
| Cargo >=1.88 | Its built-in automatic GC owns Cargo home; cleaner only reports its presence |
| Go build cache | Go automatically trims unused entries; cleaner leaves it to Go |
| Gradle caches | Gradle owns use tracking, retention and cleanup; cleaner leaves it to Gradle |
| Other package caches, browser runtimes, Devin versions | Kept; reported if present, never partially age-evicted |
| Project `target`, dependencies, environments, outputs | Optional read-only inventory; never removed by `run` |
| Custom paths, Trash, pending directories from older cleaner versions | Kept; no inferred deletion authorization |
| OS temporary files | Stock OS policy; installer never changes `/tmp` retention or runs tmpfiles cleanup |

The uv adapter resolves the executable, checks a supported release version,
asks uv for the actual cache directory and validates the destination against
`protect` and project roots. The directory must be dedicated, unredirected and
carry the standard cache marker. GC receives that exact directory explicitly,
uses offline mode, waits at most five seconds for uv's lock, and has an overall
command deadline. It never uses `--force`, `--ci`, whole-cache `clean`, or a
manual deletion fallback. Missing/old tools and protected/unmarked paths skip;
command failures remain failures in the report.

A private run lock serializes scheduled/manual cleaner runs. Output is drained
continuously but retained only up to 16 KiB per stream. Reports replace one
private `last-run.json` atomically (0700 directory / 0600 Unix file), without
archives or backups. Cache tree sizes and free-space deltas are not claimed as
reclaimed bytes; uv's own estimate is retained as text and numeric reclamation
is `null` when unknown. A dry run does not call pruning or write run state.

## Install

Choose the release for your platform and architecture, verify its `.sha256`,
extract, then run the included installer:

```sh
./install.sh        # Linux/macOS; no sudo
```

```powershell
.\install.ps1       # Windows, current user
```

Release archives contain the executable, matching installers and scheduler
assets. Source installs use `cargo build --release --locked`. The installer
preserves existing policy, validates it, installs the schedule and never starts
cleanup as an installation side effect. Windows can also fetch the explicit
`-Version 0.1.0` release when building from source is unavailable.

| OS | Schedule |
|---|---|
| Linux | daily systemd user timer, persistent catch-up, randomized delay; idle CPU/I/O priority |
| macOS | daily 03:00 launchd user agent; background CPU/I/O priority |
| Windows | daily 03:00 current-user task, missed-run catch-up, IgnoreNew instances, five-minute deadline |

`uninstall.sh`/`uninstall.ps1` remove code and the schedule, preserving policy and
last-run state. They do not edit global temporary-file policy. If an old 0.0.x
installer installed a `/tmp` override, review it separately; this installer
does not overwrite or remove another component's system configuration.

## Commands

```sh
rldyour-cleaner scan --json              # projects (if enabled) + native/cache inventory
rldyour-cleaner run --dry-run --json     # same evaluation, no GC/report write
rldyour-cleaner run --json               # supported native GC + atomic report
rldyour-cleaner status                   # last actual run, including failures
rldyour-cleaner config                   # effective policy, not hardcoded defaults
rldyour-cleaner config --init            # new annotated policy; refuses overwrite
```

`scan -v` remains accepted for compatibility; decisions are always explained.
Nothing enumerates a user's whole home by default. Cache inventory checks only
specific locations and does not compute a recursive cache size.

## Policy

| Platform | Config and state |
|---|---|
| Linux/BSD | `$XDG_CONFIG_HOME/rldyour-cleaner/config.toml`, `$XDG_STATE_HOME/rldyour-cleaner/last-run.json` (usual ~/.config / ~/.local/state fallbacks) |
| macOS | `~/Library/Application Support/rldyour-cleaner/` |
| Windows | `%LOCALAPPDATA%\rldyour-cleaner\` |

```toml
roots = []
protect = []
extra_cache_paths = []       # inventory only
command_timeout_seconds = 60 # 1..300; each discovery command is capped at 5s
pressure_pct = 101

[categories]
projects = false            # optional stale-artifact inventory under roots
incremental = true
home_caches = true
devin_versions = false
cargo_registry = false
trash = false
```

All legacy age and pressure fields still parse, but **no setting re-enables
age-based deletion**. Age fields only affect optional project inventory.
Unknown/mistyped keys fail instead of silently activating defaults. The old
mistakenly nested `[categories].extra_cache_paths` key migrates to the top-level
inventory. An old config with `projects=true`, `trash=true` or pressure enabled
cannot turn on project/trash deletion. To disable native GC, set
`categories.home_caches=false`.

`protect` matches path substrings, with leading `~` expanded. Project roots are
also excluded from native cache mutation. Paths with symlink/reparse-point
components are refused for GC and state writes; explicitly redirected caches
remain the owning tool's responsibility. Read-only project discovery never
follows symlinks or crosses filesystem boundaries; unknown mtimes are treated
as warm, not stale.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo audit
shellcheck install.sh uninstall.sh
actionlint
```

The development toolchain is pinned; MSRV is Rust 1.88. CI runs native tests and
Clippy on Linux, macOS ARM/Intel and Windows, MSRV and all release-target checks.
Integration tests use synthetic caches/private homes, including a test-only
native-tool executable. They verify previews, busy-cache failures, protected
paths, legacy configs, no deletion fallback, run locks, report permissions,
bounded output and timeouts. [Quality notes](docs/quality.md) record the primary
sources and practical limitations. No liveness snapshot can prove future
non-use, so none is used as deletion authorization.

AGPL-3.0-or-later.
