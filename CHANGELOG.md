# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-10-07

### Fixed

- Preserve uv cached environments by default, including environments linked
  from projects. The audited vendor implementation removes all cached
  environments in normal prune, not only dangling content. Legacy `uv=true`
  no longer authorizes this; a separate explicit rebuildable-environment opt-in
  is required. Symlink link mode refuses GC. Unreviewed uv versions are kept.
- Refuse directory, FIFO, redirected and oversized report destinations before
  native GC. Status reads a bounded regular file and validates cleaner identity
  before displaying it. Native output records truncation; incomplete discovery
  cannot choose a mutation destination or be treated as complete policy data.

### Validation

- Real vendor contract checks for uv 0.12.17 and 0.12.23 on CI: synthetic caches
  reproduce cached environment removal/default preservation; POSIX additionally
  tests the shared native lease and preservation of external symlink targets.
- Added regression tests for legacy opt-in, symlink link mode, unreviewed
  versions, special/oversized state, bounded status and output truncation.
- Corrected documentation to distinguish native command coordination from
  future direct interpreter use and optional centralized/cache-linked setups.

## [0.2.0] - 2026-10-07

### Added

- Native success cadence, independently keyed to provider/cache path. A daily
  scheduler has a 20-hour minimum interval tolerant of random delay. Failed GC
  never records success; clock rollback defers, malformed ledger fails closed.
  A single bounded private ledger replaces state without archives/backups;
  scan/dry-run cannot write it. Explicit `[native_gc]` policy can disable uv.
- Opt-in `apt-autoclean --enable` on Debian/Ubuntu, as root: atomically publish
  a fixed weekly obsolete-download policy, retaining installed-package archives
  and refusing to replace local edits. APT
  owns execution and package locks; no new root service, package uninstall,
  immediate cleanup or restart. Preview does not touch system state.
- Native timer/effective APT interval observations and shallow coverage for
  npx/npm logs/Corepack, browser caches, search indexes, shader caches and
  compilation caches. Unverified npm/pnpm GC stays inventory-only because a
  private cleaner lock cannot coordinate with all package installers.

### Structure and safety

- Independent uv, Node-store and inventory modules; typed actions shared by
  reporting and accounting. Separate OS policy implementations for three OSes.
- Protection is bidirectional: a native cache cannot contain a declared
  protected child or project root. Redirected report paths are refused before
  starting GC. No new runtime dependency or resident process.
- Synthetic tests cover cadence, repeat preview, failure/retry, corrupt state,
  retained runnable Node stores, protected descendants and idempotent/no-replace
  APT policy publication. Hosted CI still runs native tests on all three OSes.

## [0.1.1] - 2026-10-07

### Fixed

- Command deadlines include pipe draining. POSIX process groups and Windows
  kill-on-close Job Objects stop owned descendants after timeout or early
  parent exit; reader pipes are polled without an unbounded blocking join.
  Windows children join the job while suspended, using documented stable APIs.
- Explicit custom-config creation no longer changes existing parent directory
  permissions. Policies must be regular files <=256 KiB; unreadable cache
  metadata is reported as failure rather than absence.
- Linux ignores relative XDG paths per the base-directory specification and
  installs user units in the correct XDG config directory. Home lookup no
  longer falls back to the filesystem root. Windows resolves native EXEs only.
- macOS plist paths escape XML and shell replacement syntax independently.
  Unix and Windows installers validate policy and do not trigger cache GC.

### Structure and validation

- Separate Linux, macOS and Windows Rust modules behind one compile-time
  platform facade; shared POSIX mechanisms remain in a Unix helper. Cleanup
  policy, runner and reports contain no per-platform branches.
- Separate platform install/uninstall implementations and common Unix code
  staging; compatibility entrypoints remain at the distribution root.
- Added descendant/inherited-output regression, oversized-policy, parent-mode
  preservation, relative-XDG and synthetic installer lifecycle tests. Native
  CI covers all three OSes and both Mac architectures, with mocked schedulers
  so installer tests never register jobs or run actual cache cleanup.


## [0.1.0] - 2026-10-07

### Changed

- Automatic cleanup delegates only to supported native unused-cache GC:
  uv >=0.12.17 `cache prune`, with explicit validated cache destination,
  offline mode and native in-use locking. Cargo/Go/Gradle retain their own GC.
- Removed generic age eviction, version pruning, whole Go module-cache wipes,
  project-directory deletion and pending-directory reaping. Legacy flags and
  pressure cannot activate them. Optional artifact discovery is report-only.
- Removed unsafe process-liveness/rename assumptions, including Windows
  FILE_SHARE_DELETE and renamed Cargo lock-inode races. Defaults scan no roots.
- Separated CLI, policy, runner, native adapter, bounded commands and report
  persistence. Native output is capped at 16 KiB per stream; commands have
  deadlines and failure never falls back to manual removal.
- Scan includes cache inventory; scan/dry-run never prune or overwrite status.
  Effective `config` displays actual settings and rejects unknown keys;
  legacy misplaced extra-cache keys migrate without enabling deletion.
- Added private per-user run lock and atomic 0600 reports. Unknown reclaimed
  bytes remain null, with the native tool's estimate preserved as text.
- Installers no longer mutate global tmpfiles policy or trigger first-run
  deletion; releases include matching installers/units. Windows task runs
  as current user with bounded execution and overlap prevention.
- Native Linux/macOS ARM+Intel/Windows tests and Clippy, advisory audit,
  MSRV, cross-target and installer checks; synthetic-only deletion tests.


## [0.0.2] - 2026-09-25

### Fixed

- macOS/BSD liveness probes take a fresh `lsof` snapshot per call: the
  once-per-process cache froze the process map at first probe and could
  miss a build started mid-run (and made the suite flaky).
- Release `publish` leg resolves the repo via `-R` — it has no checkout,
  so `gh` could not find a repository and the draft stayed draft.

## [0.0.1] - 2026-09-25

### Added

- `scan` / `run` / `status` / `config` CLI (`--dry-run`, `--json`, `-v`).
- Project-artifact discovery: Rust `target` + `incremental`, `node_modules`,
  JS framework caches and outputs, flutter `build`/`.dart_tool`, gradle
  `.gradle`/`build`, python venvs and tool caches, pytest/ruff/mypy dirs.
- Six-guard safety model: exact-name + marker match, `git check-ignore` for
  generically-named dirs (`build`, `dist`, `out`) so tracked source survives,
  freshness floor, process liveness (project-root scope for dep trees), cargo
  `.cargo-lock`/`.cargo-build-lock` held through deletion, symlink/mount and
  protect-list refusal, rename-first removal with a pending-dir reaper.
- Cross-platform support: `src/os/` platform layer, per-OS config/state/
  cache dirs; schedulers = systemd --user timer (Linux), launchd agent
  (macOS, daily 03:00), Task Scheduler (Windows); liveness probe = `/proc`
  on Linux, `lsof` snapshot on macOS/BSD, mandatory file locking on Windows
  (a held tree refuses rename → reported as a skip); probe-undecidable is
  treated as in-use (fail closed).
- `$HOME` tool-cache eviction with the same liveness guard: uv, go build
  cache, bun, npm, pub, gradle, pip, pre-commit, playwright, puppeteer,
  codex-runtimes, pnpm store; devin CLI `_versions` keeps `current` only;
  `go clean -modcache` runs under disk pressure only (it wipes everything).
- Disk-pressure mode (`pressure_pct`) with tighter `[pressure]` ages —
  portable via `fs2` (`statvfs`/`GetDiskFreeSpaceEx`); `libc` dep dropped.
- Installers per OS (`install.sh` dispatch + `install.ps1`), `uninstall.*`;
  `tmpfiles.d` drop-in aging `/tmp` to 7 days (Linux).
- CI: fmt + clippy + shellcheck + plist/systemd-unit/actionlint validation
  on Linux, tests on ubuntu/macos/windows, cross-target `cargo check` for
  every release triple, MSRV 1.88 job; `rust-toolchain.toml` pins the
  dev toolchain.
- Releases (`v*` tags): tag must match `Cargo.toml` version; draft release
  published only after every leg uploads; binaries for linux-x86_64,
  linux-aarch64, macos-aarch64, macos-x86_64, windows-x86_64, each with a
  `.sha256`.

### Fixed

- `protect` patterns now expand `~` like `roots`/`extra_cache_paths` —
  `protect = ["~/x"]` previously never matched.
- `path_guard` actually re-proves a candidate's kind from its path shape
  (nested `incremental`/`flutter_build` shapes spelled out; other kinds
  round-trip through the rule table) — the comment claimed it, the code
  did not.
- macOS trash uses `~/.Trash`; the freedesktop `Trash/files` path was
  wrongly emitted under `cfg!(unix)`.
- devin `_versions` pruning fails closed when `current` is missing or
  dangling (could otherwise delete every installed version), checks
  liveness per version dir and for the `_download` staging dir, and
  removes trees rename-first — Windows can no longer half-gut a held
  version mid-`remove_dir_all`.
- The pending-dir reaper no longer crosses filesystem boundaries, matching
  the scanner's mount guard.
- `--dry-run` and real runs share one cache-spec pipeline — trash and the
  cargo registry are no longer invisible to previews.
- Reports carry per-cache `CacheResult`s plus `fresh_matched`/`stale_bytes`,
  so `status` and the JSON audit trail explain every cache decision.
