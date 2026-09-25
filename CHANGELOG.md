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
  freshness floor,
  `/proc` liveness (cwd/exe/fd/maps, project-root scope for dep trees), cargo
  `.cargo-lock`/`.cargo-build-lock` held through deletion, symlink/mount and
  protect-list refusal, rename-first removal with a pending-dir reaper.
- `$HOME` tool-cache eviction with the same `/proc` liveness guard: uv, go
  build cache, bun, npm, pub, gradle, pip, pre-commit, playwright, puppeteer,
  codex-runtimes, pnpm store; devin CLI `_versions` keeps `current` only;
  `go clean -modcache` runs under disk pressure only (it wipes everything).
- Disk-pressure mode (`pressure_pct`) with tighter `[pressure]` ages.
- systemd --user daily timer + idle-IO oneshot service; `install.sh` /
  `uninstall.sh`; `tmpfiles.d` drop-in aging `/tmp` to 7 days.
