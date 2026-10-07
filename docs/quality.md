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


## Provider/OS policy separation (2026-10-07)

Native mutation is isolated from inventory. A typed action is shared by JSON,
summary accounting and the cadence recorder, so kept/not-due/disabled cannot
be recorded as successful GC. The private 64-KiB completion ledger has bounded
entries, atomic replacement, exact provider/cache identity, clock-rollback
deferral and no historical copies. Preview never creates it; a corrupt ledger
blocks mutation. The 20-hour default avoids alternate-day skips caused by
daily scheduler jitter. Absolute protected children and roots also prevent GC
of an enclosing cache. No recursive tree-size walks are added.

The reviewed npm cacache implementation mark/sweeps content, truncates/rebuilds
index buckets and recursively removes its temporary directory; pnpm prune also
removes temporary/metadata stores and expired runnable dlx environments. Neither
provides a verified common install/GC lease for this implementation. Their
own documentation recommends native maintenance, but that alone does not prove
unattended concurrency safety. They remain read-only coverage; no process
snapshot, force flag or wrapper advisory lock pretends to coordinate unrelated
installers. Corepack shims are not invoked even for discovery. Bun only offers
a whole-cache reset or project prune, so its cache is retained.

APT automation is a separate explicit root command on Debian/Ubuntu. It publishes
one fixed weekly policy through a synced staging inode and atomic no-replace
hard link in a trusted apt.conf.d directory. A partial file is never published;
existing local edits, redirects or writable/untrusted policy paths are refused.
Normal installation/uninstallation does not change privileged system policy.
APT retains currently available downloaded archives and owns native locking;
no packages are uninstalled, no new root service or sudo grant is created.
OS timer/effective interval observation is separate for Linux/macOS/Windows.
Unknown native policy is reported as unknown, not fabricated successful cleanup.

Compared with BleachBit's extensible CleanerML and Topgrade's native-command
steps, this project retains specific provider modules and explainable previews
but excludes pattern-based cache deletion and blanket cleanup switches.
Cargo-sweep's project artifact selection is useful as an explicit build
maintenance operation, not evidence that an old target is unused.

Primary research:

- [npm cache command contract](https://docs.npmjs.com/cli/v11/commands/npm-cache/).
- [npm cacache verification implementation](https://github.com/npm/cacache/blob/main/lib/verify.js).
- [pnpm native store pruning](https://pnpm.io/cli/store).
- [pnpm 12.10.1 prune implementation](https://github.com/pnpm/pnpm/blob/v12.10.1/pnpm11/store/controller/src/storeController/prune.ts).
- [Bun cache/backend semantics](https://bun.com/docs/pm/global-cache).
- [Playwright client-tracked browser GC](https://playwright.dev/docs/browsers#stale-browser-removal).
- [APT autoclean semantics](https://github.com/Debian/apt/blob/main/doc/apt-get.8.xml).
- [BleachBit CleanerML example](https://github.com/bleachbit/bleachbit/blob/master/doc/example_cleaner.xml).
- [Topgrade native Node steps](https://github.com/topgrade-rs/topgrade/blob/main/src/steps/node.rs).
- [cargo-sweep](https://github.com/holmgr/cargo-sweep).
