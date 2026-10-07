//! POSIX mechanisms shared by the Linux and macOS implementations.
use std::{
    fs,
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::Path,
    process::{Child, Command},
};
pub trait Pipe: Read + AsRawFd {}
impl<T: Read + AsRawFd> Pipe for T {}
pub fn is_redirect(md: &fs::Metadata) -> bool {
    md.file_type().is_symlink()
}
pub fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
pub fn private_open_options(options: &mut fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}
pub fn private_permissions(path: &Path, directory: bool) -> io::Result<()> {
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
}
pub fn device_id(path: &Path) -> Option<u64> {
    fs::metadata(path).ok().map(|m| m.dev())
}
pub fn file_size(md: &fs::Metadata) -> u64 {
    if md.blocks() > 0 {
        md.blocks().saturating_mul(512)
    } else {
        md.len()
    }
}
pub fn prepare_pipe(source: &impl Pipe) -> io::Result<()> {
    let fd = source.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn read_pipe(source: &mut impl Pipe, buffer: &mut [u8]) -> io::Result<Option<usize>> {
    match source.read(buffer) {
        Ok(n) => Ok(Some(n)),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
pub struct ProcessTree {
    group: libc::pid_t,
}
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        command.process_group(0);
        let child = command.spawn()?;
        let group = child
            .id()
            .try_into()
            .map_err(|_| io::Error::other("invalid child PID"))?;
        Ok((child, Self { group }))
    }
    pub fn terminate(&mut self) {
        if self.group > 0 {
            unsafe { libc::kill(-self.group, libc::SIGKILL) };
            self.group = 0;
        }
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.terminate();
    }
}
