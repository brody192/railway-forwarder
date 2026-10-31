//! Forwarding intent is persisted separately from transient connection state.
use super::Backend;
use crate::domain::{AllowedPort, TunnelState};
use anyhow::{Result, bail};
use std::sync::Arc;

impl Backend {
    pub(super) fn start_tunnel(self: &Arc<Self>, port: AllowedPort) -> Result<()> {
        let state = self.state.lock().unwrap();
        if !state.data.signed_in || state.data.busy {
            bail!("Sign in with Railway before connecting tunnels.");
        }
        let generation = state.generation;
        let demo = state.data.demo;
        drop(state);
        if demo {
            self.tunnel_state(&port.id(), Some(TunnelState::Connected));
            return Ok(());
        }
        if let Some(attempt) = self.tunnels.begin(&port)? {
            self.tunnel_state(&port.id(), Some(TunnelState::Connecting));
            let backend = self.clone();
            let tunnels = self.tunnels.clone();
            self.runtime.spawn(async move {
                tunnels.connect(backend, port, attempt, generation).await;
            });
        }
        Ok(())
    }

    pub(super) fn restore_tunnels_on_launch(self: &Arc<Self>) {
        let ports = {
            let mut state = self.state.lock().unwrap();
            if state.startup_restore_attempted {
                return;
            }
            state.startup_restore_attempted = true;
            if !state.data.signed_in {
                return;
            }
            state.data.preferences.tunnels_to_restore()
        };
        for port in ports {
            let id = port.id();
            if let Err(error) = self.start_tunnel(port) {
                self.tunnel_state(&id, Some(TunnelState::Failed(error.to_string())));
            }
        }
    }

    pub fn tunnel_state(&self, id: &str, value: Option<TunnelState>) {
        let mut remember = false;
        self.mutate(|state| {
            // Failed attempts retain their existing intent, so a temporary
            // startup outage cannot erase the tunnels to try next launch.
            if matches!(
                value,
                Some(TunnelState::Connecting | TunnelState::Connected)
            ) && state
                .data
                .preferences
                .allowed
                .iter()
                .any(|port| port.id() == id)
                && !state
                    .data
                    .preferences
                    .active_tunnels
                    .iter()
                    .any(|saved| saved == id)
            {
                state.data.preferences.active_tunnels.push(id.into());
                state.data.preferences.active_tunnels.sort();
                remember = true;
            }
            match value {
                Some(value) => {
                    state.tunnels.insert(id.into(), value);
                }
                None => {
                    state.tunnels.remove(id);
                }
            }
        });
        if remember {
            self.save_tunnel_preferences();
        }
    }

    pub(super) fn stop(&self, id: &str) {
        self.tunnels.stop(id);
        let mut forgotten = false;
        self.mutate(|state| {
            state.tunnels.remove(id);
            state.data.preferences.active_tunnels.retain(|saved| {
                if saved == id {
                    forgotten = true;
                    false
                } else {
                    true
                }
            });
        });
        if forgotten {
            self.save_tunnel_preferences();
        }
    }

    /// Physical teardown for quitting or signing out preserves saved intent.
    pub fn stop_all(&self) {
        self.tunnels.stop_all();
        self.mutate(|state| state.tunnels.clear());
    }

    pub(super) fn disconnect_all(&self) {
        self.tunnels.stop_all();
        self.mutate(|state| {
            state.tunnels.clear();
            state.data.preferences.active_tunnels.clear();
        });
        self.save_tunnel_preferences();
    }

    fn save_tunnel_preferences(&self) {
        if let Err(error) = self.save() {
            self.mutate(|state| {
                state.data.error = Some(format!("Could not remember active tunnels. {error:#}"));
            });
        }
    }
}
