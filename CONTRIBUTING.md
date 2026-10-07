# Contributing

Issues and pull requests are welcome. For a bug report, the most useful
thing is the output of `rldyour-cleaner run --dry-run` (or `scan -v`) plus
what it got wrong — a false positive *or* a false negative matters equally.

Development conventions live in `AGENTS.md`: the module map, the safety
invariants the guards must keep, and the verify commands (`cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`, `shellcheck`,
`actionlint`). CI enforces all of them across Linux, macOS and Windows.

Changes must preserve these boundaries:

- New mutation adapters must use the owning tool's supported unused-entry GC,
  document its locking/version contract and fail without manual fallback.
- Age/liveness heuristics can improve read-only inventory, never authorize
  recursive removal or turn installed dependencies into disposable cache.
- Preview and run decisions share a pipeline; native-command failures remain
  visible. Tests use synthetic fixtures, never actual user data.
- Platform-specific implementation lives in `src/os/` or `platforms/<os>/`;
  new dependencies require a justification in Cargo.toml.
