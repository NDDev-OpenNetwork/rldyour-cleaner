# rldyour-cleaner — agent instructions

A janitor for developer machines: deletes provably-stale build artifacts and
tool caches on a systemd timer, designed so it can never break a running
build. Single binary crate, not a workspace.

## Layout

| Path | What it is |
|---|---|
| `src/kinds.rs` | artifact taxonomy: which dir names + sibling markers make a candidate, and which gate decides it |
| `src/scan.rs` | root walker, age gates, size/mtime measurement, incremental + flutter_build sub-candidates |
| `src/safety.rs` | the five guards — path shape, cargo lock, /proc liveness, freshness floor — and the held-lock `Prepared` |
| `src/clean.rs` | rename→remove_dir_all executor + `.rldyour-cleaner-pending` reaper |
| `src/homecache.rs` | $HOME tool caches: age-eviction or delegation to the tool's own GC |
| `src/config.rs` | TOML policy (`~/.config/rldyour-cleaner/config.toml`), defaults = balanced profile |
| `src/sysinfo.rs` | statvfs pressure probe, PATH lookup |
| `src/report.rs` | JSON report persisted to `$XDG_STATE_HOME/rldyour-cleaner/last-run.json` |
| `systemd/` | user units: oneshot service (Nice=19, idle IO) + daily timer |
| `tmpfiles.d/tmp.conf` | masks the stock file → /tmp aged at 7d instead of 30d |

## Invariants (do not break these)

- **Never delete anything a process uses.** If you add a kind, decide whether
  it is derived output (own-mtime gate) or a dependency tree (project-
  activity gate + project-root proc scope). Getting this backwards is the one
  way this tool can hurt someone.
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

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
shellcheck install.sh uninstall.sh
```

CI runs fmt + clippy + tests on ubuntu-latest and macos-latest (the proc
guard is Linux-only by `cfg`; everything else is portable).
