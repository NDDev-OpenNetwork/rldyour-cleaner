# Security policy

Cleanup can destroy data. This version delegates mutation only to a supported
owning-tool unused-cache GC operation; it does not delete arbitrary age-stale
files or project trees. See the README and docs/quality.md for boundaries.

Reports involving a protection bypass, force cleanup, fallback deletion,
redirected destination, unsafe native command or private report disclosure are
welcome through [GitHub private vulnerability reporting](https://github.com/NDDev-OpenNetwork/rldyour-cleaner/security/advisories/new).
Include platform, effective policy (redact private paths), version and a
synthetic reproduction. Do not send credentials or actual cached user content.

Only the latest release receives fixes. Native GC correctness depends on the
owning tool's implementation; an installed command is trusted code running as
the user. SHA256 verifies asset consistency, not publisher identity by itself.
Installers use no privilege escalation and do not alter global temp policy.
