use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Files {
    pub app: PathBuf,
    pub credentials: PathBuf,
}
impl Files {
    pub fn user() -> Result<Self> {
        let home = std::env::var_os("HOME").context("Could not locate the home directory.")?;
        let home = PathBuf::from(home);
        Ok(Self {
            app: home.join("Library/Application Support/Railway Forwarder"),
            credentials: home.join(".railway/forwarder"),
        })
    }
    pub fn settings(&self) -> PathBuf {
        self.app.join("settings.json")
    }
}
pub fn directory(path: &Path) -> Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
pub fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    read_bytes(path)?
        .map(|data| {
            serde_json::from_slice(&data)
                .context("Could not decode the saved Railway settings or session.")
        })
        .transpose()
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
            return Err(error).context("Could not read the saved Railway settings or session.");
        }
    };
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(Some(data))
}
pub fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    write_bytes(path, &serde_json::to_vec(value)?)
}
pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
    persist_bytes(path, data)
}
fn persist_bytes(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("Invalid settings path.")?;
    directory(parent)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result.context("Could not save the local Railway settings or session.")
}
pub fn delete(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Could not remove the saved Railway session."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_files_preserve_permissions_rotation_and_delete() {
        let root = std::env::temp_dir().join(format!("rf-storage-{}", uuid::Uuid::new_v4()));
        let path = root.join("tokens.json");
        write(&path, &serde_json::json!({"refreshToken":"first"})).unwrap();
        write(&path, &serde_json::json!({"refreshToken":"rotated"})).unwrap();
        assert_eq!(
            read::<serde_json::Value>(&path).unwrap().unwrap()["refreshToken"],
            "rotated"
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        delete(&path).unwrap();
        delete(&path).unwrap();
        assert!(read::<serde_json::Value>(&path).unwrap().is_none());
        fs::remove_dir(root).unwrap();
    }
}
