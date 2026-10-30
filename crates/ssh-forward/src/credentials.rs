use crate::{Config, files as storage};
use anyhow::{Context, Result, bail};
use russh::keys::{self, Algorithm, PrivateKey, PublicKey};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub(crate) struct HostKeys {
    pub(crate) path: PathBuf,
    pub(crate) lock: Mutex<()>,
}
impl HostKeys {
    pub(crate) fn verify(&self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        let _lock = self.lock.lock().unwrap();
        // Fail closed on unreadable files and symlinks before the known-hosts parser opens them.
        let saved = storage::read_bytes(&self.path)?;
        let mut key = key.clone();
        key.set_comment("");
        match keys::check_known_hosts_path(host, port, &key, &self.path) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(keys::Error::KeyChanged { line }) => bail!(
                "The SSH host key for {host} changed (known_hosts line {line}). Verify the change before updating the configured known_hosts file."
            ),
            Err(error) => return Err(error).context("Could not verify the SSH host key."),
        }
        let mut saved = saved.unwrap_or_default();
        if !saved.is_empty() && !saved.ends_with(b"\n") {
            saved.push(b'\n');
        }
        let host = if port == 22 {
            host.into()
        } else {
            format!("[{host}]:{port}")
        };
        saved.extend_from_slice(format!("{host} {}\n", key.to_openssh()?).as_bytes());
        storage::write_bytes(&self.path, &saved).context("Could not save the SSH host key.")
    }
}

pub(crate) fn identity(config: &Config) -> Result<Arc<PrivateKey>> {
    let path = config.identity_file.clone();
    let saved = storage::read_bytes(&path)?;
    let key = if let Some(saved) = saved {
        PrivateKey::from_openssh(saved).context("Could not read the SSH private key.")?
    } else {
        let mut key = PrivateKey::random(&mut rand_ssh::rng(), Algorithm::Ed25519)?;
        key.set_comment(config.key_comment.clone());
        if storage::write_new_bytes(
            &path,
            key.to_openssh(keys::ssh_key::LineEnding::LF)?.as_bytes(),
        )? {
            key
        } else {
            PrivateKey::from_openssh(
                storage::read_bytes(&path)?.context("The SSH key disappeared. Try again.")?,
            )?
        }
    };
    storage::write_bytes(
        &{
            let mut public = path.as_os_str().to_os_string();
            public.push(".pub");
            PathBuf::from(public)
        },
        key.public_key().to_openssh()?.as_bytes(),
    )?;
    Ok(Arc::new(key))
}
