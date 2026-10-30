//! Bounded file input and private state-file creation.
use crate::{Result, err, invalid, storage};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub(super) fn private_file(path: &Path) -> Result<File> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open};
        File::from(
            open(
                path,
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|_| storage())?,
        )
    };
    #[cfg(not(unix))]
    let file = {
        if let Ok(meta) = fs::symlink_metadata(path) {
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(storage());
            }
        }
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|_| storage())?
    };
    let meta = file.metadata().map_err(|_| storage())?;
    if !meta.is_file() {
        return Err(storage());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.nlink() != 1 {
            return Err(storage());
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| storage())?;
    }
    Ok(file)
}
pub(super) const MAX_TEXT: u64 = 1024 * 1024;
pub(super) const MAX_PDF: u64 = 25 * 1024 * 1024;
pub(super) fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open};
        File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| invalid())?,
        )
    };
    #[cfg(not(unix))]
    let file = {
        let meta = fs::symlink_metadata(path).map_err(|_| invalid())?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(invalid());
        }
        File::open(path).map_err(|_| invalid())?
    };
    if !file.metadata().map_err(|_| invalid())?.is_file() {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() as u64 > limit {
        return Err(err("input_limit", "Input exceeds size limit", 3));
    }
    Ok(bytes)
}
pub(super) fn text_file(path: &Path) -> Result<String> {
    String::from_utf8(read_bounded(path, MAX_TEXT)?).map_err(|_| invalid())
}
