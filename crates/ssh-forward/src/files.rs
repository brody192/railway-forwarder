use anyhow::{Context, Result};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
pub fn directory(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("Could not read the local SSH file.");
        }
    };
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(Some(data))
}
pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    persist_bytes(path, data, true).map(|_| ())
}
pub fn write_new_bytes(path: &Path, data: &[u8]) -> Result<bool> {
    persist_bytes(path, data, false)
}
fn persist_bytes(path: &Path, data: &[u8], replace: bool) -> Result<bool> {
    let parent = path.parent().context("Invalid SSH file path.")?;
    directory(parent)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<bool> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        if replace {
            fs::rename(&temporary, path)?;
        } else if let Err(error) = fs::hard_link(&temporary, path) {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Ok(false);
            }
            return Err(error.into());
        }
        Ok(true)
    })();
    let _ = fs::remove_file(temporary);
    result.context("Could not save the local SSH file.")
}
