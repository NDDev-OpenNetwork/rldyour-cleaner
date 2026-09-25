# Changelog

All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
