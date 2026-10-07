//! Non-destructive path boundaries. Cached data is changed only by its owning
//! tool's GC, never by our recursive delete or a process-liveness heuristic.
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn is_redirect(md: &fs::Metadata) -> bool {
    if md.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if md.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

/// Reject redirected components, relative paths and parent traversals. A
/// missing path is allowed for creation; an existing one must resolve wholly.
pub fn plain_path(path: &Path) -> std::io::Result<()> {
    if !path.is_absolute() {
        return Err(std::io::Error::other("path must be absolute"));
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        if matches!(part, Component::ParentDir) {
            return Err(std::io::Error::other("parent traversal refused"));
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(md) if is_redirect(&md) => {
                return Err(std::io::Error::other("redirected path refused"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn private_dir(path: &Path) -> std::io::Result<()> {
    plain_path(path)?;
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Protect both the lexical location and actual destination of native GC.
/// User-declared report roots/protect prefixes never become cache destinations.
pub fn cache_allowed(path: &Path, policy: &crate::config::Policy) -> Result<PathBuf, String> {
    plain_path(path).map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.is_dir() || canonical.parent().is_none() || canonical == crate::os::home_dir() {
        return Err("cache path is not a dedicated directory".into());
    }
    if policy
        .protect
        .iter()
        .any(|p| !p.is_empty() && canonical.to_string_lossy().contains(p))
        || policy.roots.iter().any(|root| canonical.starts_with(root))
    {
        return Err("protected path".into());
    }
    let marker = canonical.join("CACHEDIR.TAG");
    plain_path(&marker).map_err(|e| e.to_string())?;
    if !marker.symlink_metadata().is_ok_and(|md| md.is_file()) {
        return Err("cache marker is not a regular file".into());
    }
    let mut file = fs::File::open(&marker).map_err(|_| "cache marker is missing")?;
    use std::io::Read;
    let mut signature = [0u8; 43];
    file.read_exact(&mut signature)
        .map_err(|_| "cache marker is incomplete")?;
    if signature != *b"Signature: 8a477f597d28d172789f06886806bc55" {
        return Err("cache marker is not recognized".into());
    }
    Ok(canonical)
}
