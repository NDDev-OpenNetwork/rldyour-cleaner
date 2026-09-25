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
- CI: fmt + clippy + shellcheck + plist validation on Linux, tests on
  ubuntu/macos/windows, MSRV 1.88 check; `v*` tags build and publish release
  binaries for linux-x86_64, macos-aarch64, windows-x86_64.
