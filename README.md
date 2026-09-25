# rldyour-cleaner

A janitor for developer machines: it removes build artifacts and tool caches
that are **provably stale**, on a schedule, without ever breaking a build or
app that is currently running.

Not a daemon — a oneshot tool plus the OS's own scheduler. Cleanup is
periodic batch work; a sleeping process would only be one more thing to fail.

| OS | Scheduler | Temp dirs | Process liveness probe |
|---|---|---|---|
| Linux | `systemd --user` timer (daily) | `tmpfiles.d` override → `/tmp` aged at 7d | `/proc`: exe/cwd/fd/maps per candidate |
| macOS | launchd agent (daily 03:00) | built-in periodic/`/tmp` cleaner — untouched | `lsof` snapshot once per run |
| Windows | Task Scheduler (daily 03:00) | Storage Sense is the OS mechanism — untouched | mandatory file locking is the guard (rename/remove of a held tree fails → safe skip) |

## Why it can't hurt your builds

Every candidate must pass **all** of these before removal:

1. **Age gate** — derived artifacts (`target/`, `build/`, `.next/`,
   `__pycache__/`, …) must not have been written for `stale_days`
   (default 14). Dependency trees (`node_modules`, `.venv`, `.dart_tool`) are
   gated on *project* activity instead (default 30 days): their own mtime
   only moves on reinstall, so "old" is not "unused" — the project has to be
   untouched first. Activity is measured on source files (like npkill and
   cargo-sweep do), never on git history: uncommitted work still counts.
2. **Freshness floor** — anything written within `guard_fresh_minutes`
   (default 15) is off-limits, re-checked right before removal.
3. **Process guard** — no live process may hold the directory (for dep
   trees: the whole project). The probe is per-OS (`/proc`, `lsof`, or
   Windows' own locking semantics); if the platform cannot answer at all,
   the candidate is skipped — this tool fails closed, never open.
4. **Cargo lock** — for `target/` dirs, `.cargo-lock` must be acquirable
   exclusively — the same file cargo holds during builds. The lock is kept
   held *while* the tree is deleted, so a `cargo build` starting in that
   window simply waits and then creates a fresh `target/`.
5. **Path guard** — real directories only, never symlinks, never mounts
   (`st_dev` of the scan root on unix; reparse points count as symlinks on
   Windows), never paths matching `protect`, and nothing outside the
   configured `roots`.
6. **Gitignore gate** — generic names (`build/`, `dist/`, `out/`) must
   additionally be ignored by git (`git check-ignore`); a tracked `build/`
   holding source files is not an artifact, however stale it looks.

Deletion is `rename → remove_dir_all`: a process arriving mid-run sees the
directory *gone* (and recreates it cleanly) rather than half-deleted. Leftover
`.rldyour-cleaner-pending-*` dirs from a crashed run are reaped by the next.

Disk-pressure mode: when any scan root's filesystem reaches `pressure_pct`
(default 75%), tighter `[pressure]` ages apply automatically.

## What it cleans

| Area | Detection | Gate |
|---|---|---|
| `target/` (Rust) | sibling `Cargo.toml`, or `CACHEDIR.TAG` inside | artifact mtime |
| `target/*/incremental` | inside a surviving target | own mtime (shorter) |
| `node_modules/` | sibling `package.json` | project activity |
| `.next .nuxt .output .svelte-kit .turbo .parcel-cache .vite .astro coverage` | sibling `package.json` | artifact mtime |
| `dist/`, `out/` | sibling `package.json` **and** git-ignored | artifact mtime |
| `build/` | sibling `pubspec.yaml` / `*.gradle*` / `package.json` **and** git-ignored | artifact mtime |
| `.dart_tool` | sibling `pubspec.yaml` | project activity; `flutter_build` inside live ones gets its own gate |
| `.gradle` | gradle markers | artifact mtime |
| `.venv`, `venv`, `.tox`, `.nox` | python markers | project activity |
| `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.hypothesis` | anywhere | artifact mtime |
| tool caches | uv, go-build, bun, npm `_cacache`, pub, gradle, pip, pre-commit, playwright, puppeteer, codex-runtimes, pnpm store — at each OS's conventional location | per-entry age; skipped while any process uses the cache; `uv` delegates to its own GC |
| `go/pkg/mod` | go's read-only module cache | `go clean -modcache`, **pressure runs only** — it wipes everything |
| devin CLI versions | `_versions` under the devin data dir | keep `current`, drop old |

Explicitly **not** touched: `~/.cargo/registry` (cargo ≥ 1.88 GCs it itself),
`~/.android/avd`, toolchains, `.git` contents, Trash/Recycle Bin (opt-in and
unix-only), and anything matching `protect`. Temp directories are delegated
to each OS's own mechanism rather than reinvented.

## Install

```sh
./install.sh       # Linux: systemd --user timer · macOS: launchd agent
.\install.ps1      # Windows: Task Scheduler task
```

Everything lands under the user's home/profile; the only privileged step is
Linux's optional `/etc/tmpfiles.d/tmp.conf` drop-in (skipped with a printed
recipe if sudo is unavailable). `uninstall.sh` / `uninstall.ps1` reverse it.

## Commands

```sh
rldyour-cleaner scan            # table of stale candidates — never deletes
rldyour-cleaner scan -v         # include skipped entries with reasons
rldyour-cleaner scan --json     # machine-readable
rldyour-cleaner run --dry-run   # full evaluation, no mutation
rldyour-cleaner run             # what the scheduler calls
rldyour-cleaner status          # last run's JSON report
rldyour-cleaner config          # effective policy; --init writes the file
```

## Policy

`config.toml` under the platform config dir — Linux
`~/.config/rldyour-cleaner/`, macOS `~/Library/Application Support/`,
Windows `%LOCALAPPDATA%\` — every key optional; the shipped default file
documents them all. Highlights:

```toml
roots = ["~/Developer"]
stale_days = 14          # derived artifacts
dep_stale_days = 30      # dependency dirs (project-activity gate)
incremental_days = 7     # target/*/incremental
cache_entry_days = 30    # files inside tool caches
guard_fresh_minutes = 15
pressure_pct = 75        # >100 disables pressure mode entirely
protect = []             # path substrings never deleted
extra_cache_paths = []   # extra dirs evicted like tool caches
```

## Repository layout

```
src/
  kinds.rs     artifact taxonomy: names + sibling markers → kind
  scan.rs      root walker, age gates, measurement, sub-candidates
  safety.rs    the guards + the held-lock `Prepared`
  clean.rs     rename→remove executor + pending-dir reaper
  homecache.rs tool caches: age-evict or delegate to the tool's own GC
  config.rs    TOML policy, balanced defaults
  report.rs    JSON run report for `status`
  os/          everything OS-specific, one file per platform:
    mod.rs       dirs/paths, PATH lookup, fs fill, device & size helpers
    linux.rs     /proc liveness probe
    unix_lsof.rs lsof liveness probe (macOS, BSDs)
    windows.rs   locking-semantics guard (probe = the delete itself)
platforms/
  linux/       systemd units + tmpfiles.d override
  macos/       launchd plist (@HOME@ substituted at install)
install.ps1 / uninstall.ps1   Windows — registers a Task Scheduler task
```

## Verify / develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
shellcheck install.sh uninstall.sh
actionlint                                     # workflow semantics
systemd-analyze verify platforms/linux/systemd/*   # on Linux
```

`rust-toolchain.toml` pins stable + rustfmt/clippy for everyone; MSRV is
`rust-version` in `Cargo.toml` and is exercised in CI against 1.88.0.

## CI / release

CI lints (fmt, clippy, shellcheck, actionlint, plist + systemd-unit
verification), runs the tests on ubuntu/macos/windows, and `cargo check`s
every release target so cross-compile bugs surface in the PR, not at tag
time.

A `v*` tag publishes binaries — the release stays a **draft** until all
legs upload, so a failed build never ships a partial release. The tag must
equal `version` in `Cargo.toml`.

```sh
git tag -s v0.0.1 -m v0.0.1 && git push --tags
```

Assets: `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `macos-x86_64`,
`windows-x86_64`, each with a `.sha256` checksum.
