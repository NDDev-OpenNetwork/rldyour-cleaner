# Cleanup boundaries and research — 2026-10-07

Age is a candidate-discovery heuristic, not proof of non-use. Package stores
can contain hardlinks referenced by installed environments; applications may
load resources lazily without holding a descriptor. Windows FILE_SHARE_DELETE
allows deleting/renaming an opened file. Renaming a target directory also moves
its lock inode; a new build can recreate the old pathname and lock a different
file. These mechanisms cannot provide the previous absolute deletion guarantee.
Consequently this version removes those mutation paths rather than trying to
cover the race with another snapshot or a shorter freshness interval.

The uv adapter uses `cache prune`, never direct cache edits. uv documents
append-only cache safety, periodic prune and cache-modification locking. Version
0.12.17 is the lowest version qualified for this adapter; unknown/prerelease
strings are kept. Prune can remove centralized cached environments which uv
recreates as needed; it does not prune ordinary project environments. The
cache marker validates a dedicated cache path, but is not evidence that every
file within it is disposable. uv decides what can be removed.

Cargo tracks download/source use and automatically cleans its cache since
1.88. Go trims build-cache entries automatically. Gradle tracks use and owns
retention of its distributions and shared/project caches. They remain managed
by their owners. Browser profiles, server data, logs, projects, model downloads,
installed tools and Trash receive no new deletion policy from this utility.

Default runtime work is shallow inventory plus native tool invocations. It
avoids whole-cache measurement and expensive per-candidate `lsof`/`/proc`
walks. Run and preview share decisions, but preview does not invoke prune;
exact reclaimable bytes cannot be predicted. GC runs offline without force,
with a five-second uv lock wait and 1–300-second command deadline (default 60).
A killed timed-out GC may already have removed some owner-approved unused
entries; the failure is reported and no generic cleanup follows it. Installed
native tools are trusted not to leave subprocesses holding output pipes after
exit; bounded readers cover the qualified direct uv commands.

Private run lock and atomic reports are local to one user, not a distributed
protocol. Unix modes are explicit; Windows inherits its user's local profile
ACL. Configuration and native tools must remain under the owner's control.

Tests are synthetic: native-adapter CLI, failure/busy refusal, no fallback,
protected/unmarked/redirected paths, version floor, overlap, private atomic
reports, correct effective config, legacy compatibility and bounded output.
CI exercises real Linux/macOS ARM+Intel/Windows and every release target.

Primary sources checked:

- [uv cache safety, prune and lock timeout](https://docs.astral.sh/uv/concepts/cache/).
- [Cargo automatic cache cleanup](https://doc.rust-lang.org/cargo/reference/config.html#cache).
- [Go build-cache management](https://pkg.go.dev/cmd/go#hdr-Build_and_test_caching).
- [Gradle use-aware cache cleanup](https://docs.gradle.org/current/userguide/directory_layout.html#sec:automatic_cleanup).
- [Windows CreateFile sharing and delete semantics](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
- [Rust std::fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html).
- [Stable Rust distribution manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml).
