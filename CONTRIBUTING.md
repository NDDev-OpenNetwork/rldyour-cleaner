# Contributing

Issues and pull requests are welcome. For a bug report, the most useful
thing is the output of `rldyour-cleaner run --dry-run` (or `scan -v`) plus
what it got wrong — a false positive *or* a false negative matters equally.

Development conventions live in `AGENTS.md`: the module map, the safety
invariants the guards must keep, and the verify commands (`cargo fmt`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`, `shellcheck`,
`actionlint`). CI enforces all of them across Linux, macOS and Windows.

Rules that will get a PR bounced:

- A removal path without its guard story — every new candidate kind needs
  to say which gate makes it safe (artifact mtime vs project activity) and
  why a process can't be mid-write inside it.
- Platform-specific code outside `src/os/` and `platforms/<os>/`.
- New dependencies without a line of justification in `Cargo.toml`.
