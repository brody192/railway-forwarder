use std::{path::PathBuf, time::Duration};
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// SSH endpoint and explicit locations for the identity and known-hosts pins.
#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub identity_file: PathBuf,
    pub known_hosts_file: PathBuf,
    pub key_comment: String,
    pub connect_timeout: Duration,
}
impl Config {
    /// Uses SSH port 22 and a 20-second connection timeout by default.
    pub fn new(
        host: impl Into<String>,
        identity_file: impl Into<PathBuf>,
        known_hosts_file: impl Into<PathBuf>,
    ) -> Self {
        Self {
            host: host.into(),
            port: 22,
            identity_file: identity_file.into(),
            known_hosts_file: known_hosts_file.into(),
            key_comment: "ssh-forward".into(),
            connect_timeout: CONNECT_TIMEOUT,
        }
    }
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    pub fn key_comment(mut self, comment: impl Into<String>) -> Self {
        self.key_comment = comment.into();
        self
    }
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }
}
/// The destination reached from the SSH server, including IPv6 destinations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemotePort {
    pub host: String,
    pub port: u16,
}
impl RemotePort {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}
