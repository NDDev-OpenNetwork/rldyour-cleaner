# rldyour-cleaner

A janitor for developer machines: it removes build artifacts and tool caches
that are **provably stale**, on a schedule, without ever breaking a build or
app that is currently running.

Not a daemon — a oneshot tool plus a `systemd --user` timer. Cleanup is
periodic batch work; a sleeping process would only be one more thing to fail.

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
3. **Process guard** — on Linux no process may have the directory (for dep
   trees: the whole project) as its `cwd`, `exe`, an open fd, or a memory
   mapping (a gradle daemon's jars only show up in `maps`).
4. **Cargo lock** — for `target/` dirs, `.cargo-lock` must be acquirable
   exclusively — the same file cargo holds during builds. The lock is kept
   held *while* the tree is deleted, so a `cargo build` starting in that
   window simply waits and then creates a fresh `target/`.
5. **Path guard** — real directories only, never symlinks, never mounts
   (`st_dev` of the scan root), never paths matching `protect`, and nothing
   outside the configured `roots`.
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
| `.gradle`, `android/app/build` | gradle markers | artifact mtime |
| `.venv`, `venv`, `.tox`, `.nox` | python markers | project activity |
| `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.hypothesis` | anywhere | artifact mtime |
| tool caches | `~/.cache/{uv,go-build,pip,pre-commit,ms-playwright,puppeteer,codex-runtimes}`, `~/.bun/install/cache`, `~/.npm/_cacache`, `~/.pub-cache`, `~/.gradle/caches`, pnpm store | per-entry age; skipped while any process uses the cache; `uv` delegates to its own GC |
| `~/go/pkg/mod` | go's read-only module cache | `go clean -modcache`, **pressure runs only** — it wipes everything |
| devin CLI versions | `~/.local/share/devin/cli/_versions` | keep `current`, drop old |

Explicitly **not** touched: `~/.cargo/registry` (cargo ≥ 1.88 GCs it itself),
`~/.android/avd`, toolchains under `~/.local/share/rldyour`, `~/.git` contents,
Trash (opt-in), and anything matching `protect`. `/tmp` is handled by the OS:
`install.sh` drops a `tmpfiles.d` override aging entries at 7 days (distro
default is 30).

## Install

```sh
./install.sh      # build → ~/.local/bin, units → systemd --user, enable timer
```

Everything lands under `$HOME`; the only root step is the optional
`/etc/tmpfiles.d/tmp.conf` drop-in (skipped with a printed recipe if sudo is
unavailable). `uninstall.sh` reverses it and restores the stock /tmp policy.

## Commands

```sh
rldyour-cleaner scan            # table of stale candidates — never deletes
rldyour-cleaner scan -v         # include skipped entries with reasons
rldyour-cleaner scan --json     # machine-readable
rldyour-cleaner run --dry-run   # full evaluation, no mutation
rldyour-cleaner run             # what the timer calls
rldyour-cleaner status          # last run's JSON report
rldyour-cleaner config          # effective policy; --init writes the file
```

## Policy

`~/.config/rldyour-cleaner/config.toml` — every key optional; the shipped
default file documents them all. Highlights:

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

## Verify / develop

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
shellcheck install.sh uninstall.sh
systemd-analyze --user verify systemd/*.service systemd/*.timer  # if systemd present
```
