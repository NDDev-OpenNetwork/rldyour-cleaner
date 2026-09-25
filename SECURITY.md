# Security Policy

This tool **deletes files** — that is the feature, so a bug in it can be
data loss. The safety model (age gates, process liveness, lock interlocks,
rename-first removal, fail-closed probes) is documented in the README; if
you find a way it can remove something it shouldn't, that report is
especially welcome.

## Reporting a vulnerability

- Preferred: [GitHub private vulnerability reporting](https://github.com/NDDev-OpenNetwork/rldyour-cleaner/security/advisories/new)
- Otherwise: mail `danil@nddev.it.com` with `[rldyour-cleaner]` in the
  subject.

Please include the platform, the policy in effect (`rldyour-cleaner config`),
what was deleted (or would be, via `run --dry-run`), and why that was wrong.

## Scope notes

- "It deleted a directory" is only a bug if a guard should have stopped it —
  stale `target/` dirs are exactly what it removes.
- The install scripts verify release assets by SHA-256; a compromised
  download is rejected, not installed.
- The tool never touches credentials, `.git` contents, or paths matching
  `protect`; it cannot escalate privileges (the only privileged step is an
  optional, user-invoked `sudo install` of a tmpfiles drop-in on Linux).

## Supported versions

Only the latest release receives fixes.
