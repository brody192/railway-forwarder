use super::Backend;
use crate::domain::{AllowedPort, MenuPreferences, Port, TunnelState, clean_name};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Command {
    action: String,
    id: Option<String>,
    value: Option<String>,
    enabled: Option<bool>,
    kind: Option<String>,
    port: Option<Port>,
    #[serde(rename = "targetID")]
    target_id: Option<String>,
    #[serde(rename = "workspaceID")]
    workspace_id: Option<String>,
    #[serde(rename = "projectID")]
    project_id: Option<String>,
    #[serde(rename = "environmentID")]
    environment_id: Option<String>,
    #[serde(rename = "serviceID")]
    service_id: Option<String>,
}
impl Backend {
    pub fn dispatch(self: &Arc<Self>, value: Value) {
        let result = serde_json::from_value(value)
            .map_err(anyhow::Error::from)
            .and_then(|command| self.command(command));
        if let Err(error) = result {
            self.mutate(|state| state.data.error = Some(format!("{error:#}")));
        }
    }
    fn command(self: &Arc<Self>, command: Command) -> Result<()> {
        let id = command.id.as_deref().unwrap_or("");
        let value = command.value.as_deref().unwrap_or("");
        let generation = self.state.lock().unwrap().generation;
        match command.action.as_str() {
            "quit" | "disconnect" => self.stop_all(),
            "showMenu" => crate::app::events::show_menu(),
            "dismissError" => self.mutate(|state| state.data.error = None),
            "refreshAccount" => {
                if !self.state.lock().unwrap().data.demo {
                    let backend = self.clone();
                    self.runtime.spawn(async move {
                        backend.refresh_account(generation).await;
                    });
                }
            }
            "searchCatalog" => {
                if command.enabled == Some(true) {
                    self.request_search(value);
                } else {
                    self.cancel_search();
                }
            }
            "displayName" => {
                let snapshot = self.snapshot();
                if !snapshot.display_names.iter().any(|label| label.key == id) {
                    bail!("This resource is no longer available.");
                }
                let value = clean_name(value, "");
                self.mutate(|state| {
                    if value.is_empty() {
                        state.data.preferences.menu.names.remove(id);
                    } else {
                        state.data.preferences.menu.names.insert(id.into(), value);
                    }
                });
                self.save()?;
            }
            "menuReset" => {
                self.mutate(|state| state.data.preferences.menu = MenuPreferences::default());
                self.save()?;
            }
            "menuMove" => {
                let snapshot = self.snapshot();
                let target = command.target_id.as_deref().unwrap_or("");
                let kind = command.kind.as_deref().unwrap_or("");
                let mut order: Vec<_> = if kind == "section" {
                    snapshot
                        .menu
                        .sections
                        .iter()
                        .map(|section| section.id.clone())
                        .collect()
                } else {
                    snapshot.menu.sections.iter().find(|section|section.ports.iter().any(|port|port.id==id)&&section.ports.iter().any(|port|port.id==target)).context("Ports can be reordered within their project. Drag a project heading to move the whole section.")?.ports.iter().map(|port|port.id.clone()).collect()
                };
                let from = order
                    .iter()
                    .position(|item| item == id)
                    .context("This menu item is no longer available.")?;
                let to = order
                    .iter()
                    .position(|item| item == target)
                    .context("This menu item is no longer available.")?;
                let item = order.remove(from);
                order.insert(to, item);
                self.mutate(|state| {
                    if kind == "section" {
                        state.data.preferences.menu.section_order = order;
                    } else {
                        state
                            .data
                            .preferences
                            .menu
                            .port_order
                            .retain(|id| !order.contains(id));
                        state.data.preferences.menu.port_order.extend(order);
                    }
                });
                self.save()?;
            }
            "clientID" => {
                self.mutate(|state| {
                    if !state.data.signed_in {
                        state.data.preferences.client_id = value.trim().into();
                    }
                });
                self.save()?;
            }
            "localPort" => {
                let port = port_number(value)?;
                let old = self
                    .state
                    .lock()
                    .unwrap()
                    .data
                    .preferences
                    .allowed
                    .iter()
                    .find(|port| port.id() == id)
                    .cloned()
                    .context("This port is no longer available.")?;
                if old.local_port != port {
                    self.stop(id);
                    self.mutate(|state| {
                        if let Some(item) = state
                            .data
                            .preferences
                            .allowed
                            .iter_mut()
                            .find(|item| item.id() == id)
                        {
                            item.local_port = port;
                        }
                    });
                    self.save()?;
                }
            }
            "forwardPort" => {
                let snapshot = self.snapshot();
                let port = snapshot
                    .preferences
                    .allowed
                    .iter()
                    .find(|port| port.id() == id)
                    .cloned()
                    .context("This port is no longer available.")?;
                if command.enabled == Some(true) {
                    if snapshot.demo {
                        self.tunnel_state(id, Some(TunnelState::Connected));
                    } else {
                        match self.tunnels.begin(&port) {
                            Ok(Some(attempt)) => {
                                self.tunnel_state(id, Some(TunnelState::Connecting));
                                let backend = self.clone();
                                let tunnels = self.tunnels.clone();
                                self.runtime.spawn(async move {
                                    tunnels.connect(backend, port, attempt, generation).await;
                                });
                            }
                            Ok(None) => {}
                            Err(error) => {
                                self.tunnel_state(id, Some(TunnelState::Failed(error.to_string())))
                            }
                        }
                    }
                } else {
                    self.stop(id);
                }
            }
            "refresh" => {
                self.pause_search();
                if !self.state.lock().unwrap().data.demo {
                    let backend = self.clone();
                    self.runtime.spawn(async move {
                        backend.refresh(generation).await;
                    });
                }
            }
            "signIn" => self.sign_in(),
            "signOut" => self.sign_out(),
            _ => {
                let state = self.state.lock().unwrap();
                let workspace = state
                    .data
                    .workspaces
                    .iter()
                    .find(|workspace| Some(&workspace.id) == command.workspace_id.as_ref())
                    .context("The selected workspace is no longer available.")?
                    .clone();
                let project = workspace
                    .projects
                    .iter()
                    .find(|project| Some(&project.id) == command.project_id.as_ref())
                    .context(
                        "The selected project is no longer available. Refresh the project list.",
                    )?
                    .clone();
                let demo = state.data.demo;
                drop(state);
                if command.action == "loadProject" {
                    if !demo {
                        let backend = self.clone();
                        self.runtime.spawn(async move {
                            backend
                                .load(project, command.enabled == Some(true), generation)
                                .await;
                        });
                    }
                    return Ok(());
                }
                let environment = project
                    .environments
                    .iter()
                    .find(|env| Some(&env.id) == command.environment_id.as_ref())
                    .context("The selected environment is no longer available.")?
                    .clone();
                let service = environment
                    .services
                    .iter()
                    .find(|service| Some(&service.id) == command.service_id.as_ref())
                    .context("The selected service is no longer available. Refresh the project.")?
                    .clone();
                let key = format!("{}/{}", environment.id, service.id);
                match command.action.as_str() {
                    "scanService" => {
                        if !demo {
                            let backend = self.clone();
                            self.runtime.spawn(async move {
                                backend.scan(environment, service, generation).await;
                            });
                        }
                    }
                    "addManual" => match port_number(value) {
                        Ok(port) => self.mutate(|state| {
                            let ports = state.data.ports.entry(key.clone()).or_default();
                            let port = Port {
                                host: "127.0.0.1".into(),
                                port,
                            };
                            if !ports.contains(&port) {
                                ports.push(port);
                                ports.sort_by_key(|port| port.port);
                            }
                            state.data.node_errors.remove(&key);
                        }),
                        Err(error) => self.mutate(|state| {
                            state.data.node_errors.insert(key, error.to_string());
                        }),
                    },
                    "allowPort" => {
                        let port = command
                            .port
                            .context("Select a discovered port or add a port manually.")?;
                        let state = self.state.lock().unwrap();
                        if port.port == 0
                            || !(state
                                .data
                                .ports
                                .get(&key)
                                .is_some_and(|ports| ports.contains(&port))
                                || state
                                    .data
                                    .preferences
                                    .allowed
                                    .iter()
                                    .any(|item| item.service_key() == key && item.remote == port))
                        {
                            bail!("Select a discovered port or add a port manually.");
                        }
                        drop(state);
                        let item = AllowedPort {
                            workspace_id: workspace.id,
                            workspace_name: workspace.name,
                            project_id: project.id,
                            project_name: project.name,
                            environment_id: environment.id,
                            environment_name: environment.name,
                            service_id: service.id,
                            service_name: service.name,
                            local_port: port.port,
                            remote: port,
                        };
                        if command.enabled != Some(true) {
                            self.stop(&item.id());
                        }
                        self.mutate(|state| {
                            if command.enabled == Some(true) {
                                if !state
                                    .data
                                    .preferences
                                    .allowed
                                    .iter()
                                    .any(|port| port.id() == item.id())
                                {
                                    state.data.preferences.allowed.push(item);
                                }
                            } else {
                                state
                                    .data
                                    .preferences
                                    .allowed
                                    .retain(|port| port.id() != item.id());
                            }
                        });
                        self.save()?;
                    }
                    _ => bail!("Unknown settings action."),
                }
            }
        }
        Ok(())
    }
}
fn port_number(value: &str) -> Result<u16> {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .context("Enter a TCP port between 1 and 65535.")
}
