//! Owns application state, persisted preferences and background task lifetimes.
mod catalog;
mod commands;
mod demo;
mod diagnostics;
mod search;
mod session;
#[cfg(test)]
mod tests;
mod tunnels;
use crate::{
    domain::{Preferences, Snapshot, TunnelState, display_names, menu_layout, project_catalog},
    integrations::railway::OAuth,
    integrations::railway::Railway,
    integrations::ssh::{Ssh, Tunnels},
    persistence::{self, Files},
};
use anyhow::Result;
use demo::seed_demo;
use search::{SearchRequest, SearchTask};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::{runtime::Runtime, task::JoinHandle};
use uuid::Uuid;

struct State {
    data: Snapshot,
    generation: u64,
    loads: HashMap<String, Uuid>,
    tunnels: HashMap<String, TunnelState>,
    search_request: Option<SearchRequest>,
    startup_restore_attempted: bool,
}
pub struct Backend {
    state: Mutex<State>,
    files: Files,
    runtime: Runtime,
    pub api: Railway,
    pub ssh: Ssh,
    pub tunnels: Arc<Tunnels>,
    search: Mutex<Option<SearchTask>>,
    auth: Mutex<Option<JoinHandle<()>>>,
}
struct Loading {
    backend: Weak<Backend>,
    key: String,
    id: Uuid,
    generation: u64,
}
impl Drop for Loading {
    fn drop(&mut self) {
        if let Some(backend) = self.backend.upgrade() {
            let mut state = backend.state.lock().unwrap();
            if state.generation == self.generation && state.loads.get(&self.key) == Some(&self.id) {
                state.loads.remove(&self.key);
            }
            drop(state);
            crate::app::events::changed();
        }
    }
}
impl Backend {
    pub fn new(demo: bool) -> Result<Arc<Self>> {
        Self::with_files(demo, Files::user()?)
    }
    fn with_files(demo: bool, files: Files) -> Result<Arc<Self>> {
        let (preferences, error) = if demo {
            (Preferences::default(), None)
        } else {
            match persistence::read(&files.settings()) {
                Ok(preferences) => (preferences.unwrap_or_default(), None),
                Err(error) => (Preferences::default(), Some(format!("{error:#}"))),
            }
        };
        let (oauth, signed_in, auth_error) =
            match OAuth::new(files.clone(), &preferences.client_id, !demo) {
                Ok((oauth, signed_in)) => (oauth, signed_in, None),
                Err(error) => {
                    let (oauth, _) = OAuth::new(files.clone(), &preferences.client_id, false)?;
                    (oauth, false, Some(format!("{error:#}")))
                }
            };
        let mut data = Snapshot {
            preferences,
            demo,
            signed_in,
            error: error.or(auth_error),
            ..Default::default()
        };
        if demo {
            seed_demo(&mut data);
        }
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                data,
                generation: 0,
                loads: HashMap::new(),
                tunnels: HashMap::new(),
                search_request: None,
                startup_restore_attempted: false,
            }),
            api: Railway {
                oauth: Arc::new(oauth),
            },
            ssh: Ssh::new(files.clone()),
            files,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(3)
                .enable_all()
                .build()?,
            tunnels: Arc::new(Tunnels::new()),
            search: Mutex::new(None),
            auth: Mutex::new(None),
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        let state = self.state.lock().unwrap();
        let mut data = state.data.clone();
        let allowed = if data.signed_in {
            data.preferences.allowed.as_slice()
        } else {
            &[]
        };
        let names = display_names(&data.workspaces, allowed, &data.preferences.menu.names);
        data.menu = menu_layout(allowed, &data.preferences.menu, &names, &state.tunnels);
        data.workspaces = project_catalog(&data.workspaces, &names);
        data.display_names = names.into_values().collect();
        data.display_names.sort_by(|a, b| a.key.cmp(&b.key));
        data.loading = state.loads.keys().cloned().collect();
        data.loading.sort();
        data
    }
    pub fn login_item_status(&self) -> Option<crate::domain::LoginItemStatus> {
        let state = self.state.lock().unwrap();
        (!state.data.demo).then_some(state.data.login_item)
    }
    pub fn update_login_item(&self, status: crate::domain::LoginItemStatus, error: Option<String>) {
        self.mutate(|state| {
            state.data.login_item = status;
            match error {
                Some(error) => {
                    state.data.node_errors.insert("login-item".into(), error);
                }
                None => {
                    state.data.node_errors.remove("login-item");
                }
            }
        });
    }
    fn mutate(&self, update: impl FnOnce(&mut State)) {
        update(&mut self.state.lock().unwrap());
        crate::app::events::changed();
    }
    fn save(&self) -> Result<()> {
        let state = self.state.lock().unwrap();
        if !state.data.demo {
            persistence::write(&self.files.settings(), &state.data.preferences)?;
        }
        Ok(())
    }
    fn loading(self: &Arc<Self>, key: &str, generation: u64) -> Option<Loading> {
        let mut state = self.state.lock().unwrap();
        if state.generation != generation || state.loads.contains_key(key) {
            return None;
        }
        let id = Uuid::new_v4();
        state.loads.insert(key.into(), id);
        drop(state);
        crate::app::events::changed();
        Some(Loading {
            backend: Arc::downgrade(self),
            key: key.into(),
            id,
            generation,
        })
    }
    pub fn shutdown(&self) {
        self.mutate(|state| state.generation += 1);
        if let Some(task) = self.auth.lock().unwrap().take() {
            task.abort();
        }
        self.cancel_search();
        self.stop_all();
    }
}
