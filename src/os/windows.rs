//! Windows profile paths and native process-tree ownership. A process starts
//! suspended, joins a private kill-on-close Job Object, then its primary
//! thread resumes. Descendants cannot race ahead of job assignment.
use std::{
    fs,
    io::{self, Read},
    mem::size_of,
    os::windows::{fs::MetadataExt, io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_BROKEN_PIPE, HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Pipes::PeekNamedPipe,
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
        },
    },
};
pub fn home_dir() -> PathBuf {
    super::resolved_home("USERPROFILE")
}
fn local() -> PathBuf {
    super::absolute_env("LOCALAPPDATA").unwrap_or_else(|| home_dir().join("AppData/Local"))
}
pub fn config_dir() -> PathBuf {
    local().join("rldyour-cleaner")
}
pub fn state_dir() -> PathBuf {
    config_dir()
}
pub fn cache_dir() -> PathBuf {
    local()
}
pub fn data_local_dir() -> PathBuf {
    local()
}
pub fn executable_names(name: &str) -> Vec<String> {
    vec![format!("{name}.exe")]
}
pub fn is_executable(path: &Path) -> bool {
    path.is_file()
}
pub fn is_redirect(md: &fs::Metadata) -> bool {
    md.file_type().is_symlink() || md.file_attributes() & 0x400 != 0
}
pub fn private_open_options(_options: &mut fs::OpenOptions) {}
pub fn private_permissions(_path: &Path, _directory: bool) -> io::Result<()> {
    Ok(())
} // user's inherited LocalAppData ACL
pub fn device_id(_path: &Path) -> Option<u64> {
    None
}
pub fn file_size(md: &fs::Metadata) -> u64 {
    md.len()
}
pub trait Pipe: Read + AsRawHandle {}
impl<T: Read + AsRawHandle> Pipe for T {}
pub fn prepare_pipe(_source: &impl Pipe) -> io::Result<()> {
    Ok(())
}
pub fn read_pipe(source: &mut impl Pipe, buffer: &mut [u8]) -> io::Result<Option<usize>> {
    let mut available = 0;
    if unsafe {
        PeekNamedPipe(
            source.as_raw_handle(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut available,
            ptr::null_mut(),
        )
    } == 0
    {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
            Ok(Some(0))
        } else {
            Err(e)
        };
    }
    if available == 0 {
        return Ok(None);
    }
    let count = buffer.len().min(available as usize);
    source.read(&mut buffer[..count]).map(Some)
}
struct Handle(HANDLE);
impl Handle {
    fn checked(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
pub struct ProcessTree {
    job: Handle,
}
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        let job = Handle::checked(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) })?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        let mut child = command.spawn()?;
        let mut tree = Self { job };
        if let Err(e) = tree.assign_and_resume(&child) {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        Ok((child, tree))
    }
    fn assign_and_resume(&self, child: &Child) -> io::Result<()> {
        if unsafe { AssignProcessToJobObject(self.job.0, child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // std's primary-thread handle API is nightly-only. Documented ToolHelp
        // + OpenThread/ResumeThread keeps this crate on stable Rust.
        let snapshot = Handle::checked(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
        let mut thread: THREADENTRY32 = unsafe { std::mem::zeroed() };
        thread.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut found = unsafe { Thread32First(snapshot.0, &mut thread) };
        while found != 0 {
            if thread.th32OwnerProcessID == child.id() {
                let handle = Handle::checked(unsafe {
                    OpenThread(THREAD_SUSPEND_RESUME, 0, thread.th32ThreadID)
                })?;
                match unsafe { ResumeThread(handle.0) } {
                    1 => return Ok(()),
                    0 => {} // not the suspended primary thread
                    u32::MAX => return Err(io::Error::last_os_error()),
                    _ => return Err(io::Error::other("unexpected primary-thread suspend count")),
                }
            }
            found = unsafe { Thread32Next(snapshot.0, &mut thread) };
        }
        Err(io::Error::other(
            "suspended child's primary thread not found",
        ))
    }
    pub fn terminate(&mut self) {
        unsafe { TerminateJobObject(self.job.0, 1) };
    }
}
