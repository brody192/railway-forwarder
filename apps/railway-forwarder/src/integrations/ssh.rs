use crate::{
    domain::{AllowedPort, Port, TunnelState},
    integrations::railway::Railway,
    persistence::Files,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;
use uuid::Uuid;

pub struct Ssh {
    client: ssh_forward::Client,
    ready: tokio::sync::Mutex<Option<u64>>,
}
impl Ssh {
    pub fn new(files: Files) -> Self {
        Self {
            client: ssh_forward::Client::new(
                ssh_forward::Config::new(
                    "ssh.railway.com",
                    files.app.join("id_ed25519"),
                    files.app.join("known_hosts"),
                )
                .key_comment("Railway Forwarder"),
            ),
            ready: tokio::sync::Mutex::new(None),
        }
    }
    pub async fn prepare(&self, api: &Railway, generation: u64) -> Result<()> {
        let mut ready = self.ready.lock().await;
        if *ready != Some(generation) {
            api.register_key(&self.client.public_key()?).await?;
            *ready = Some(generation);
        }
        Ok(())
    }
    fn target(target: &str) -> Result<()> {
        Uuid::parse_str(target).context("Railway returned an invalid service instance ID.")?;
        Ok(())
    }
    pub async fn scan(&self, target: &str) -> Result<Vec<Port>> {
        Self::target(target)?;
        self.client.discover_ports(target).await
    }
    pub async fn probe(&self, target: &str, remote: &Port) -> Result<()> {
        Self::target(target)?;
        self.client.probe(target, remote).await
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
struct Attempt {
    id: Uuid,
    local: u16,
    cancel: watch::Sender<bool>,
}
pub struct Tunnels {
    attempts: Mutex<HashMap<String, Attempt>>,
}
impl Tunnels {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }
    pub fn begin(&self, port: &AllowedPort) -> Result<Option<Uuid>> {
        let mut attempts = self.attempts.lock().unwrap();
        if attempts.contains_key(&port.id()) {
            return Ok(None);
        }
        if attempts
            .values()
            .any(|attempt| attempt.local == port.local_port)
        {
            bail!(
                "Another tunnel already uses local port {}. Change its local port in settings.",
                port.local_port
            );
        }
        let id = Uuid::new_v4();
        let (cancel, _) = watch::channel(false);
        attempts.insert(
            port.id(),
            Attempt {
                id,
                local: port.local_port,
                cancel,
            },
        );
        Ok(Some(id))
    }
    pub fn stop(&self, key: &str) {
        if let Some(attempt) = self.attempts.lock().unwrap().remove(key) {
            attempt.cancel.send_replace(true);
        }
    }
    pub fn stop_all(&self) {
        for (_, attempt) in self.attempts.lock().unwrap().drain() {
            attempt.cancel.send_replace(true);
        }
    }
    fn fail(&self, backend: &crate::app::backend::Backend, key: &str, id: Uuid, error: String) {
        let mut attempts = self.attempts.lock().unwrap();
        if !attempts.get(key).is_some_and(|attempt| attempt.id == id) {
            return;
        }
        if let Some(attempt) = attempts.remove(key) {
            attempt.cancel.send_replace(true);
        }
        backend.tunnel_state(key, Some(TunnelState::Failed(error)));
    }
    fn connected(&self, backend: &crate::app::backend::Backend, key: &str, id: Uuid) -> bool {
        let attempts = self.attempts.lock().unwrap();
        if !attempts.get(key).is_some_and(|attempt| attempt.id == id) {
            return false;
        }
        backend.tunnel_state(key, Some(TunnelState::Connected));
        true
    }
    pub async fn connect(
        self: Arc<Self>,
        backend: Arc<crate::app::backend::Backend>,
        port: AllowedPort,
        id: Uuid,
        generation: u64,
    ) {
        let key = port.id();
        let cancel = self
            .attempts
            .lock()
            .unwrap()
            .get(&key)
            .filter(|attempt| attempt.id == id)
            .map(|attempt| attempt.cancel.subscribe());
        let Some(mut cancel) = cancel else {
            return;
        };
        let result = cancellable(&mut cancel, async {
            backend.ssh.prepare(&backend.api, generation).await?;
            let target = backend
                .api
                .instance(&port.environment_id, &port.service_id)
                .await?;
            Ssh::target(&target)?;
            let mut tunnel = backend
                .ssh
                .client
                .forward(
                    &target,
                    ([127, 0, 0, 1], port.local_port).into(),
                    port.remote,
                )
                .await?;
            if !self.connected(&backend, &key, id) {
                return Ok(());
            }
            tunnel.wait().await
        })
        .await;
        if let Some(Err(error)) = result {
            self.fail(&backend, &key, id, format!("{error:#}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservations_cancel_pending_attempts_and_release_local_ports() {
        let tunnels = Tunnels::new();
        let first = AllowedPort {
            local_port: 5432,
            remote: Port {
                host: "127.0.0.1".into(),
                port: 5432,
            },
            ..Default::default()
        };
        let mut other = first.clone();
        other.service_id = "other".into();
        assert!(tunnels.begin(&first).unwrap().is_some());
        assert!(tunnels.begin(&first).unwrap().is_none());
        assert!(tunnels.begin(&other).is_err());
        let receiver = tunnels.attempts.lock().unwrap()[&first.id()]
            .cancel
            .subscribe();
        tunnels.stop(&first.id());
        assert!(*receiver.borrow());
        assert!(tunnels.begin(&other).unwrap().is_some());
        tunnels.stop_all();
        assert!(tunnels.attempts.lock().unwrap().is_empty());
    }
}
