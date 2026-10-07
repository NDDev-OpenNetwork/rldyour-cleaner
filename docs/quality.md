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
entries; the failure is reported and no generic cleanup follows it. Commands own a process group on POSIX and a kill-on-close Job Object on Windows.
Windows spawn is suspended until assignment completes, so children cannot race
out of job containment. Documented ToolHelp/OpenThread/ResumeThread replace the
nightly-only Rust primary-thread accessor. Nonblocking POSIX pipe reads and
single-reader Windows pipe availability checks bound output draining; timeout
cancels readers and terminates the owned tree. Unix groups assume trusted tools
do not deliberately detach with setsid; the pipe deadline still prevents an
escaped writer from hanging the CLI. Kernel-level uninterruptible operations
remain outside a userspace deadline's guarantee.


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


## Platform contract and installer validation

Linux follows XDG absolute-path/default rules; relative XDG values are ignored.
macOS uses per-user Library paths and launchd StartCalendarInterval (sleep wake
catch-up; power-off misses wait until the next slot). Windows uses LocalAppData
and Task Scheduler catch-up/IgnoreNew, with native EXE-only resolution. These
are three independent compile-time modules behind a small facade, sharing
POSIX primitives only where OS semantics actually match.

Unix installer tests use a synthetic home containing spaces, Unicode and `&`,
custom XDG config, real code/config operations and fake scheduler commands.
Windows installer tests mock Task Scheduler cmdlets but execute actual
install/upgrade/uninstall against a synthetic profile. CI verifies their
arguments and policy preservation; it does not claim an interactive Windows
user's scheduled logon session was exercised on the hosted runner. Actual Mac
launchd registration and Ubuntu user timer are verified on the owner's devices.

Additional primary references checked 2026-10-07:

- [XDG path rules](https://specifications.freedesktop.org/basedir/latest/).
- [Rust POSIX process groups](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.process_group).
- [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).
- [AssignProcessToJobObject and suspended creation](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject).
- [PeekNamedPipe semantics](https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-peeknamedpipe).
- [Apple scheduling and sleep/power-off behavior](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html).
- [Microsoft Task Scheduler settings](https://learn.microsoft.com/en-us/powershell/module/scheduledtasks/new-scheduledtasksettingsset).
- [Systemd timer upstream reference](https://github.com/systemd/systemd/blob/main/man/systemd.timer.xml).
