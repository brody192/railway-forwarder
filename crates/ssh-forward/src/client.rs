use crate::{
    Config, RemotePort, Tunnel,
    credentials::{self, HostKeys},
    discovery,
};
use anyhow::{Context, Result, bail};
use russh::{
    client,
    keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKey},
};
use std::{
    net::{Shutdown, TcpStream as StdTcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::watch,
    time::timeout,
};
pub(crate) const CHANNEL_TIMEOUT: Duration = Duration::from_secs(15);
/// A reusable SSH endpoint with an explicit identity and host-key store.
pub struct Client {
    config: Config,
    pub(super) hosts: Arc<HostKeys>,
}
impl Client {
    /// Creates a client without connecting or reading credentials.
    pub fn new(config: Config) -> Self {
        Self {
            hosts: Arc::new(HostKeys {
                path: config.known_hosts_file.clone(),
                lock: Mutex::new(()),
            }),
            config,
        }
    }
    /// Returns the OpenSSH public key, generating an owner-only Ed25519 identity if absent.
    pub fn public_key(&self) -> Result<String> {
        Ok(self.identity()?.public_key().to_openssh()?)
    }
    pub(super) fn identity(&self) -> Result<Arc<PrivateKey>> {
        credentials::identity(&self.config)
    }
    async fn connect(&self, username: &str) -> Result<Connection> {
        self.connect_to(username, &self.config.host, self.config.port)
            .await
    }
    /// Binds a local socket and authenticates SSH before returning a live tunnel.
    /// Local port zero asks the OS to choose an available port.
    pub async fn forward(
        &self,
        username: &str,
        local: std::net::SocketAddr,
        remote: RemotePort,
    ) -> Result<Tunnel> {
        let listener = TcpListener::bind(local)
            .await
            .with_context(|| format!("Could not bind local address {local}."))?;
        let connection = self.connect(username).await?;
        Tunnel::start(listener, connection, remote)
    }
    async fn connect_to(&self, target: &str, host: &str, port: u16) -> Result<Connection> {
        let key = self.identity()?;
        timeout(self.config.connect_timeout, async {
            let stream = TcpStream::connect((host, port))
                .await
                .context("Could not connect to the SSH server.")?;
            stream.set_nodelay(true)?;
            let stream = stream.into_std()?;
            // Dropping/canceling a connection shuts down its socket even if russh still
            // owns background tasks or individual channel handles.
            let transport = Transport(stream.try_clone()?);
            let stream = TcpStream::from_std(stream)?;
            let (closed, receiver) = watch::channel(None);
            let handler = SessionHandler {
                hosts: self.hosts.clone(),
                host: host.into(),
                port,
                closed,
            };
            let config = client::Config {
                keepalive_interval: Some(Duration::from_secs(15)),
                keepalive_max: 3,
                nodelay: true,
                ..Default::default()
            };
            let mut handle = client::connect_stream(Arc::new(config), stream, handler)
                .await
                .context("Could not establish the SSH connection.")?;
            let hash = if key.algorithm().is_rsa() {
                handle.best_supported_rsa_hash().await?.flatten()
            } else {
                None
            };
            if !handle
                .authenticate_publickey(target, PrivateKeyWithHashAlg::new(key, hash))
                .await?
                .success()
            {
                bail!("The SSH server rejected the configured identity.");
            }
            Ok(Connection {
                handle: Arc::new(handle),
                closed: receiver,
                _transport: transport,
            })
        })
        .await
        .context("Timed out connecting to the SSH server.")?
    }
    /// Reads Linux TCP listeners over SFTP without executing a remote command.
    /// Requires access to `/proc/net/tcp` or `/proc/net/tcp6` on the SSH server.
    pub async fn discover_ports(&self, target: &str) -> Result<Vec<RemotePort>> {
        discovery::discover(self.connect(target).await?).await
    }
    /// Authenticates and opens a direct TCP channel without sending application data.
    pub async fn probe(&self, target: &str, remote: &RemotePort) -> Result<()> {
        let connection = self.connect(target).await?;
        let result = timeout(
            CHANNEL_TIMEOUT,
            connection.handle.channel_open_direct_tcpip(
                remote.host.clone(),
                u32::from(remote.port),
                "127.0.0.1",
                0,
            ),
        )
        .await
        .context("The forwarded connection timed out.")?;
        match result {
            Ok(channel) => {
                channel.close().await?;
                connection.close().await;
                Ok(())
            }
            Err(error) => {
                connection.close().await;
                Err(error.into())
            }
        }
    }
}

pub(crate) struct SessionHandler {
    hosts: Arc<HostKeys>,
    host: String,
    port: u16,
    closed: watch::Sender<Option<String>>,
}
impl client::Handler for SessionHandler {
    type Error = anyhow::Error;
    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool> {
        self.hosts.verify(&self.host, self.port, key)?;
        Ok(true)
    }
    async fn disconnected(&mut self, reason: client::DisconnectReason<Self::Error>) -> Result<()> {
        let message = match reason {
            client::DisconnectReason::ReceivedDisconnect(info) => {
                if info.message.is_empty() {
                    "The SSH tunnel closed. Toggle it on to reconnect.".into()
                } else {
                    info.message
                }
            }
            client::DisconnectReason::Error(error) => format!("The SSH tunnel closed. {error:#}"),
        };
        self.closed.send_replace(Some(message));
        Ok(())
    }
}
struct Transport(StdTcpStream);
impl Drop for Transport {
    fn drop(&mut self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
}
pub(crate) struct Connection {
    pub(crate) handle: Arc<client::Handle<SessionHandler>>,
    pub(crate) closed: watch::Receiver<Option<String>>,
    _transport: Transport,
}
impl Connection {
    pub(crate) async fn close(&self) {
        let _ = timeout(
            Duration::from_secs(1),
            self.handle
                .disconnect(russh::Disconnect::ByApplication, "", "en"),
        )
        .await;
    }
}
