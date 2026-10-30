use crate::{
    RemotePort,
    client::{CHANNEL_TIMEOUT, Connection},
};
use anyhow::{Context, Result, bail};
use std::future::Future;
use tokio::{io::copy_bidirectional, net::TcpListener, sync::watch, task::JoinSet, time::timeout};
const MAX_CONNECTIONS: usize = 256;
/// Owns a listener and its SSH session. Dropping it cancels forwarding.
/// Use `close().await` to wait until sockets and client tasks have been released.
pub struct Tunnel {
    local_addr: std::net::SocketAddr,
    stop: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<Result<()>>>,
}
impl Tunnel {
    pub(crate) fn start(
        listener: TcpListener,
        connection: Connection,
        remote: RemotePort,
    ) -> Result<Self> {
        let local_addr = listener.local_addr()?;
        let (stop, mut cancel) = watch::channel(false);
        let task = tokio::spawn(async move {
            cancellable(&mut cancel, forward(listener, connection, remote))
                .await
                .unwrap_or(Ok(()))
        });
        Ok(Self {
            local_addr,
            stop,
            task: Some(task),
        })
    }

    /// Returns the bound address, including an OS-assigned local port.
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.local_addr
    }
    /// Requests shutdown without waiting for socket cleanup.
    pub fn stop(&self) {
        self.stop.send_replace(true);
    }
    /// Waits for shutdown or a transport error. Canceling this future keeps ownership intact.
    pub async fn wait(&mut self) -> Result<()> {
        let Some(task) = self.task.as_mut() else {
            return Ok(());
        };
        let result = task.await;
        self.task.take();
        result.context("The forwarding task failed.")?
    }
    /// Stops forwarding and waits for the listener, connections and SSH session to close.
    pub async fn close(mut self) -> Result<()> {
        self.stop();
        self.wait().await
    }
}
impl Drop for Tunnel {
    fn drop(&mut self) {
        self.stop();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    while !*cancel.borrow_and_update() {
        if cancel.changed().await.is_err() {
            return;
        }
    }
}
async fn cancellable<F: Future>(cancel: &mut watch::Receiver<bool>, work: F) -> Option<F::Output> {
    tokio::select! {biased; _ = cancelled(cancel) => None, result = work => Some(result)}
}
async fn forward(listener: TcpListener, connection: Connection, remote: RemotePort) -> Result<()> {
    let mut closed = connection.closed.clone();
    let mut clients = JoinSet::new();
    loop {
        if let Some(error) = closed.borrow().as_ref() {
            bail!("{error}");
        }
        tokio::select! {
            biased;
            result = closed.changed() => {
                if result.is_err() {bail!("The SSH connection ended. Toggle it on to reconnect.");}
            }
            _ = clients.join_next(), if !clients.is_empty() => {}
            accepted = listener.accept(), if clients.len() < MAX_CONNECTIONS => {
                let (mut socket, origin) = accepted.context("Could not accept a local connection.")?;
                let session = connection.handle.clone();
                let remote = remote.clone();
                clients.spawn(async move {
                    let channel = timeout(CHANNEL_TIMEOUT, session.channel_open_direct_tcpip(
                        remote.host, u32::from(remote.port), origin.ip().to_string(), u32::from(origin.port())
                    )).await.context("The forwarded connection timed out.")??;
                    let mut stream = channel.into_stream();
                    copy_bidirectional(&mut socket, &mut stream).await?;
                    Ok::<_, anyhow::Error>(())
                });
            }
        }
    }
}
