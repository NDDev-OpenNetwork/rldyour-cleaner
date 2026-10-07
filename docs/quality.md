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
0.12.17 and 0.12.23 are the two qualified versions for this adapter; other
release/prerelease strings are kept. Prune removes all centralized/cached environments, including targets
referenced by project links. A regular project-local environment is outside the
cache, but symlink-installed dependencies can still depend on cache files. The
default policy therefore never invokes prune; a separate explicit opt-in is
required for rebuildable environments after reviewing those dependencies. The
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
- [APT autoclean semantics](https://manpages.ubuntu.com/manpages/resolute/man8/apt-get.8.html).
- [BleachBit CleanerML example](https://github.com/bleachbit/bleachbit/blob/master/doc/example_cleaner.xml).
- [Topgrade native Node steps](https://github.com/topgrade-rs/topgrade/blob/main/src/steps/node.rs).
- [cargo-sweep](https://github.com/holmgr/cargo-sweep).


## Preservation audit (2026-10-07)

The original vendor-GC assumption needed a narrower contract. In both audited
uv versions, `Cache::prune` explicitly removes every `environments-v2` entry.
The native cache lease serializes active uv commands; it cannot protect direct
Python uses outside uv or preserve linked environments for future/offline use.
A synthetic real-vendor test reproduces the dangling project-environment link
after prune, while default cleaner policy refuses the operation. This is an
intentional preservation correction, not a broader deletion adapter. The older
external-symlink escape (#19542/#19543) is fixed in the two audited releases and
is tested with synthetic victim content that must survive.

`uv_prune_rebuildable_environments=false` is the compatibility/default boundary:
legacy uv=true alone never enables prune. Explicit opt-in has documented scope;
UV_LINK_MODE=symlink refuses it, and unknown vendor releases are kept. Absence of
a symlink setting is not evidence about existing dependencies. User cache
contents or global project trees are never scanned to invent that proof.

Reports are at most 4 MiB and destinations must be regular files before GC.
Directory/FIFO/redirected/oversized destinations fail without native mutation.
Status validates the cleaner identity instead of printing arbitrary file
contents. Discovery output truncation is explicit and fails closed for native
version/path/system-policy decisions. These bounds improve observable failure
without adding runtime dependencies, cache walks or resident polling.

Primary evidence:

- [Exact uv 0.12.17 cache implementation](https://github.com/astral-sh/uv/blob/0.12.17/crates/uv-cache/src/lib.rs).
- [Exact uv 0.12.23 cache implementation](https://github.com/astral-sh/uv/blob/0.12.23/crates/uv-cache/src/lib.rs).
- [Centralized project environments](https://docs.astral.sh/uv/concepts/projects/layout/#centralized-project-environments).
- [Symlink link-mode caveat](https://docs.astral.sh/uv/reference/settings/#link-mode).
- [External-symlink escape fix](https://github.com/astral-sh/uv/pull/19543).
- [Installed Ubuntu APT 3.2 documentation](https://manpages.ubuntu.com/manpages/resolute/man8/apt-get.8.html).

The real-vendor checks use installed official uv on the two devices and pinned
setup-uv releases on CI. Only temporary synthetic cache entries are deleted.
POSIX lease/symlink tests do not claim corresponding Windows link behavior;
Windows exercises cached-environment removal/default preservation and the
Rust Job Object/regression tests independently.


## System-maintenance closure (2026-10-07)

The next audit checked the whole execution chain, not only a present timer.
Ubuntu's installed apt.systemd.daily invokes autoclean from its `install`
branch: apt-daily-upgrade owns it, while apt-daily is the update/download
branch. The periodic enable default is 1, an explicit 0 disables everything,
and Clean-Installed must remain false for installed archive preservation.
Doctor observes effective values without evaluating shell output and removes
user APT_CONFIG overrides when querying the system policy. It separately
reports native timer state, wrapper result and autoclean-stamp metadata.
The wrapper can exit successfully after skipping work, so no single one of
those observations is presented as proof that all maintenance completed.

Doctor has its own read-only pipeline and never enters runner/native GC,
creates a state directory, obtains a cleaner run lock or overwrites status.
It diagnoses saved failure/preview/stale/future/older-version reports, valid
completion state and opt-in environment removal. Linux uses one user-manager
snapshot, macOS one own-agent launchctl snapshot, and Windows fixed native
Get-ScheduledTask/Get-ScheduledTaskInfo queries scoped to the root task.
Only whitelisted fields reach the report; native scheduler output is bounded,
raw environment/action dumps are not echoed, and incomplete formats stay
unknown. No observer changes or starts native jobs.

Redirected cache ancestors now stop shallow inventory too. Windows install
routes all validate policy with staged new code before replacing the installed
executable. A synthetic PE with an appended marker makes unintended overwrite
observable in the invalid-policy lifecycle test. Existing user policy, job
registration and code are preserved on that refusal.

Primary references:

- [APT configuration queries/types](https://manpages.ubuntu.com/manpages/resolute/man8/apt-config.8.html).
- Installed Ubuntu `/usr/lib/apt/apt.systemd.daily`, apt-daily.service and
  apt-daily-upgrade.service (exact host package implementation, read-only).
- [systemd timer lifecycle](https://github.com/systemd/systemd/blob/main/man/systemd.timer.xml).
- [Apple launchd jobs/lifecycle](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html).
- [Microsoft task runtime information](https://learn.microsoft.com/en-us/powershell/module/scheduledtasks/get-scheduledtaskinfo).

Native command formats can change or access may be unavailable; doctor reports
that uncertainty instead of fabricating a healthy job. A successful diagnostic
is scoped to these observed checks, not a guarantee about every application
cache, the whole OS, future data use, or hardware reliability.
