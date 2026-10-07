//! One private advisory lock serializes this user's scheduled/manual runs.
//! No project directory is renamed or removed by this module.
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub struct RunLock {
    _file: File,
}
impl RunLock {
    pub fn acquire(state_dir: &Path) -> std::io::Result<Self> {
        crate::safety::private_dir(state_dir)?;
        let path = state_dir.join("run.lock");
        if let Ok(md) = path.symlink_metadata()
            && (!md.is_file() || crate::safety::is_redirect(&md))
        {
            return Err(std::io::Error::other("run lock is not a regular file"));
        }
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        crate::os::private_open_options(&mut options);
        let file = options.open(path)?;
        file.try_lock_exclusive()?;
        Ok(Self { _file: file })
    }
}
