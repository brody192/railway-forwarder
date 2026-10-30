use super::*;
use crate::{
    credentials::HostKeys,
    discovery::{listener_rows, parse_ports},
    files as storage,
};
use anyhow::Result;
use russh::keys::{Algorithm, PrivateKey, PublicKey};
use russh::server;
use russh_sftp::protocol::{Attrs, Data, FileAttributes, Handle, OpenFlags, Status, StatusCode};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::timeout,
};
use uuid::Uuid;
const USER: &str = "00000000-0000-0000-0000-000000000001";
const TABLE: &str = "  sl local_address rem_address st\n 0: 00000000:1538 00000000:0000 0A\n 1: 00000000:1F90 00000000:0000 01\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("rf-ssh-{}", Uuid::new_v4())))
    }
    fn config(&self) -> Config {
        Config::new(
            "127.0.0.1",
            self.0.join("id_ed25519"),
            self.0.join("known_hosts"),
        )
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Job(tokio::task::JoinHandle<()>);
impl Drop for Job {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[test]
fn identity_and_host_key_pins_are_preserved() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let scratch = Scratch::new();
    let ssh = Client::new(scratch.config());
    let first = ssh.identity().unwrap();
    let saved = storage::read_bytes(&scratch.0.join("id_ed25519"))
        .unwrap()
        .unwrap();
    assert_eq!(first.public_key(), ssh.identity().unwrap().public_key());
    assert_eq!(
        saved,
        storage::read_bytes(&scratch.0.join("id_ed25519"))
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        std::fs::metadata(scratch.0.join("id_ed25519"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    ssh.hosts
        .verify("example.test", 22, first.public_key())
        .unwrap();
    ssh.hosts
        .verify("example.test", 22, first.public_key())
        .unwrap();
    let known = storage::read_bytes(&ssh.hosts.path).unwrap();
    let changed = PrivateKey::random(&mut rand_ssh::rng(), Algorithm::Ed25519).unwrap();
    assert!(
        ssh.hosts
            .verify("example.test", 22, changed.public_key())
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(known, storage::read_bytes(&ssh.hosts.path).unwrap());
    let link = scratch.0.join("symlink");
    symlink(&ssh.hosts.path, &link).unwrap();
    let symlinked = HostKeys {
        path: link,
        lock: Mutex::new(()),
    };
    assert!(
        symlinked
            .verify("example.test", 22, first.public_key())
            .is_err()
    );
    let listeners = parse_ports(&listener_rows("/proc/net/tcp", TABLE));
    assert_eq!(
        listeners,
        vec![RemotePort {
            host: "127.0.0.1".into(),
            port: 5432
        }]
    );
}

struct Fixture {
    key: PublicKey,
    destination: SocketAddr,
    sessions: Arc<Mutex<Vec<server::Handle>>>,
    channels: HashMap<russh::ChannelId, russh::Channel<server::Msg>>,
}
impl server::Handler for Fixture {
    type Error = anyhow::Error;
    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<server::Auth> {
        Ok(if user == USER && key.key_data() == self.key.key_data() {
            server::Auth::Accept
        } else {
            server::Auth::reject()
        })
    }
    async fn auth_succeeded(&mut self, session: &mut server::Session) -> Result<()> {
        self.sessions.lock().unwrap().push(session.handle());
        Ok(())
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: russh::Channel<server::Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        _: &mut server::Session,
    ) -> Result<bool> {
        if host != "127.0.0.1" || port != u32::from(self.destination.port()) {
            return Ok(false);
        }
        let mut socket = TcpStream::connect(self.destination).await?;
        tokio::spawn(async move {
            let _ = copy_bidirectional(&mut socket, &mut channel.into_stream()).await;
        });
        Ok(true)
    }
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<server::Msg>,
        _: &mut server::Session,
    ) -> Result<bool> {
        self.channels.insert(channel.id(), channel);
        Ok(true)
    }
    async fn subsystem_request(
        &mut self,
        id: russh::ChannelId,
        name: &str,
        session: &mut server::Session,
    ) -> Result<()> {
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let channel = self.channels.remove(&id).unwrap();
        session.channel_success(id)?;
        tokio::spawn(russh_sftp::server::run(channel.into_stream(), Tables));
        Ok(())
    }
}
struct Tables;
impl russh_sftp::server::Handler for Tables {
    type Error = StatusCode;
    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> std::result::Result<Handle, StatusCode> {
        if filename != "/proc/net/tcp" || flags.bits() != OpenFlags::READ.bits() {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle {
            id,
            handle: filename,
        })
    }
    async fn fstat(&mut self, id: u32, _: String) -> std::result::Result<Attrs, StatusCode> {
        // Kernel tables report size zero despite having readable contents.
        Ok(Attrs {
            id,
            attrs: FileAttributes {
                size: Some(0),
                ..Default::default()
            },
        })
    }
    async fn read(
        &mut self,
        id: u32,
        _: String,
        offset: u64,
        len: u32,
    ) -> std::result::Result<Data, StatusCode> {
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Eof)?;
        if offset >= TABLE.len() {
            return Err(StatusCode::Eof);
        }
        Ok(Data {
            id,
            data: TABLE.as_bytes()[offset..TABLE.len().min(offset + len as usize)].to_vec(),
        })
    }
    async fn close(&mut self, id: u32, _: String) -> std::result::Result<Status, StatusCode> {
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: "en".into(),
        })
    }
}
async fn round_trip(address: SocketAddr, data: Vec<u8>) -> Result<()> {
    timeout(Duration::from_secs(10), async {
        let mut client = TcpStream::connect(address).await?;
        client.write_all(&data).await?;
        client.shutdown().await?;
        let mut result = Vec::new();
        client.read_to_end(&mut result).await?;
        let expected: Vec<_> = data.into_iter().rev().collect();
        assert_eq!(result.len(), expected.len());
        assert!(
            result == expected,
            "The forwarded bytes differ from the remote response."
        );
        Ok::<_, anyhow::Error>(())
    })
    .await?
}
#[tokio::test]
async fn native_transport_forwards_discovers_and_cleans_up_without_processes() {
    let scratch = Scratch::new();
    let ssh = Client::new(scratch.config());
    let key = ssh.identity().unwrap().public_key().clone();
    let echo = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let destination = echo.local_addr().unwrap();
    let _echo = Job(tokio::spawn(async move {
        loop {
            let (mut socket, _) = echo.accept().await.unwrap();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                if socket.read_to_end(&mut bytes).await.is_ok() {
                    bytes.reverse();
                    let _ = socket.write_all(&bytes).await;
                    let _ = socket.shutdown().await;
                }
            });
        }
    }));
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let original = PrivateKey::random(&mut rand_ssh::rng(), Algorithm::Ed25519).unwrap();
    let changed = PrivateKey::random(&mut rand_ssh::rng(), Algorithm::Ed25519).unwrap();
    let rotate = Arc::new(AtomicBool::new(false));
    let rotate_server = rotate.clone();
    let sessions = Arc::new(Mutex::new(Vec::new()));
    let server_sessions = sessions.clone();
    let _server = Job(tokio::spawn(async move {
        let mut connections = JoinSet::new();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let config = server::Config {
                keys: vec![if rotate_server.load(Ordering::SeqCst) {
                    changed.clone()
                } else {
                    original.clone()
                }],
                auth_rejection_time: Duration::ZERO,
                ..Default::default()
            };
            let handler = Fixture {
                key: key.clone(),
                destination,
                sessions: server_sessions.clone(),
                channels: HashMap::new(),
            };
            connections.spawn(async move {
                if let Ok(session) = server::run_stream(Arc::new(config), stream, handler).await {
                    let _ = session.await;
                }
            });
        }
    }));
    let ssh = Client::new(scratch.config().port(endpoint.port()));
    let ports = ssh.discover_ports(USER).await.unwrap();
    assert_eq!(ports, vec![RemotePort::new("127.0.0.1", 5432)]);
    let remote = RemotePort::new(destination.ip().to_string(), destination.port());
    let tunnel = ssh
        .forward(USER, "127.0.0.1:0".parse().unwrap(), remote.clone())
        .await
        .unwrap();
    let address = tunnel.local_addr();
    let (a, b) = tokio::join!(
        round_trip(address, vec![7; 1024 * 1024]),
        round_trip(address, (0..131071).map(|v| (v % 251) as u8).collect())
    );
    a.unwrap();
    b.unwrap();
    let mut idle = TcpStream::connect(address).await.unwrap();
    idle.write_all(b"waiting").await.unwrap();
    timeout(Duration::from_secs(2), tunnel.close())
        .await
        .unwrap()
        .unwrap();
    let read = timeout(Duration::from_secs(2), idle.read_u8())
        .await
        .unwrap();
    assert!(read.is_err());
    let released = TcpListener::bind(address).await.unwrap();
    drop(released);
    let mut tunnel = ssh.forward(USER, address, remote.clone()).await.unwrap();
    let handle = sessions.lock().unwrap().last().unwrap().clone();
    handle
        .disconnect(
            russh::Disconnect::ByApplication,
            "fixture closed".into(),
            "en".into(),
        )
        .await
        .unwrap();
    let error = timeout(Duration::from_secs(2), tunnel.wait())
        .await
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("fixture closed"));
    let dropped = ssh
        .forward(USER, "127.0.0.1:0".parse().unwrap(), remote.clone())
        .await
        .unwrap();
    let released_address = dropped.local_addr();
    drop(dropped);
    timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(listener) = TcpListener::bind(released_address).await {
                drop(listener);
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    rotate.store(true, Ordering::SeqCst);
    let error = ssh.probe(USER, &remote).await.unwrap_err();
    assert!(format!("{error:#}").contains("host key"));
}
#[test]
fn listener_parser_preserves_targets_and_ignores_invalid_rows() {
    let ports = parse_ports(
        "/proc/net/tcp 00000000:1538\n/proc/net/tcp 00000000:1538\n/proc/net/tcp 0200000A:1F90\n/proc/net/tcp6 00000000000000000000000000000000:0BB8\n/proc/net/tcp6 00000000000000000000000001000000:2382\n/proc/net/tcp6 ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ:1538\n/proc/net/tcp invalid\n/proc/net/tcp 00000000:0000",
    );
    assert_eq!(
        ports,
        vec![
            RemotePort {
                host: "::1".into(),
                port: 3000
            },
            RemotePort {
                host: "127.0.0.1".into(),
                port: 5432
            },
            RemotePort {
                host: "10.0.0.2".into(),
                port: 8080
            },
            RemotePort {
                host: "::1".into(),
                port: 9090
            }
        ]
    );
}
