# rldyour-cleaner — agent instructions

A janitor for developer machines: deletes provably-stale build artifacts and
tool caches on a daily OS-scheduled run, designed so it can never break a
running build. Single binary crate, not a workspace. Platforms: Linux,
macOS, Windows — anything OS-specific lives under `src/os/` (one module per
platform) and `platforms/<os>/` (scheduler + installer assets).

## Layout

| Path | What it is |
|---|---|
| `src/kinds.rs` | artifact taxonomy: which dir names + sibling markers make a candidate, and which gate decides it |
| `src/scan.rs` | root walker, age gates, size/mtime measurement, incremental + flutter_build sub-candidates |
| `src/safety.rs` | the guards — path shape, cargo lock, process liveness, freshness floor — and the held-lock `Prepared` |
| `src/clean.rs` | rename→remove_dir_all executor + `.rldyour-cleaner-pending` reaper; `ExecuteError::Busy` is the Windows "in use" verdict |
| `src/homecache.rs` | tool caches: age-eviction or delegation to the tool's own GC; specs list every plausible per-OS path |
| `src/config.rs` | TOML policy under `os::config_dir()`, defaults = balanced profile |
| `src/os/` | platform layer — shared helpers in `mod.rs`, liveness per OS |
| `src/os/linux.rs` | `/proc` probe (exe/cwd/fd/maps), per-call |
| `src/os/unix_lsof.rs` | `lsof` snapshot probe — macOS + BSDs |
| `src/os/windows.rs` | no enumeration; Windows file locking + `Busy` mapping IS the guard |
| `src/report.rs` | JSON report persisted to `os::state_dir()/last-run.json` |
| `platforms/linux/` | systemd units + tmpfiles.d override (/tmp 30d→7d) |
| `platforms/macos/` | launchd plist, `@HOME@` templated |
| `install.sh` / `uninstall.sh` | Linux + macOS dispatcher (`install -m`, not `-D` — GNU-only) |
| `install.ps1` / `uninstall.ps1` | Windows — Task Scheduler via `Register-ScheduledTask` (StartWhenAvailable ≈ systemd `Persistent`) |

## Invariants (do not break these)

- **Never delete anything a process uses.** If you add a kind, decide whether
  it is derived output (own-mtime gate) or a dependency tree (project-
  activity gate + project-root proc scope). Getting this backwards is the one
  way this tool can hurt someone.
- **`pids_using` is fail-closed.** `None` (platform cannot enumerate) means
  "in use" → skip. Never turn an undecidable probe into an empty list.
- **Windows' guard is its locking semantics**: a held tree refuses rename —
  `clean::execute` maps that to `Busy` → reported as a skip, never a failure.
- **Locks are held through deletion.** `Prepared._hold_lock` must outlive the
  rename+remove; dropping it early reopens the race it exists to close.
- **Never create files inside a candidate as part of a guard** — it bumps the
  dir mtime and the next freshness check sees our own write. Lock files are
  only opened if they already exist.
- **`walkdir`: `filter_entry` hides entries entirely; `skip_current_dir`
  yields but does not descend.** Artifact dirs need the latter. The pending-
  dir reaper must descend `target/` and `.dart_tool/` (they host nested
  candidates) — its prune set is `SKIP_DIRS` minus those two containers.
- **Generic dir names (`build`, `dist`, `out`) require `git check-ignore`** —
  name + marker alone matched a tracked `build/` of pin files once. Kinds
  with `needs_gitignore()` are candidates only when git answers "ignored";
  "git can't say" counts as keep.
- Deletion is rename-first (`rename → remove_dir_all`), never direct rm.
- No following symlinks, no crossing filesystem boundaries, `protect`
  substrings are absolute.
- Deps stay minimal and justified in `Cargo.toml`. English everywhere.
- Public repo: no host/user/estate facts in code, config or docs.
- Temp dirs are the OS's job — tmpfiles.d on Linux, built-in cleaners on
  macOS/Windows. The tool never rm's `/tmp` itself.

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings          # linux
cargo check --all-targets --target aarch64-apple-darwin
cargo check --all-targets --target x86_64-pc-windows-msvc
cargo check --all-targets --target aarch64-unknown-linux-gnu
cargo test
shellcheck install.sh uninstall.sh
actionlint
```

CI runs fmt + clippy + shellcheck + plist/systemd-unit/actionlint
validation on Linux, tests on ubuntu/macos/windows, cross-target `cargo
check` for every release triple, and an MSRV (1.88) job. Tag pushes (`v*`)
build a draft release that is published only after all five platform legs
upload (linux x86_64+aarch64, macos aarch64+x86_64, windows x86_64); the
tag must equal `version` in `Cargo.toml`.
