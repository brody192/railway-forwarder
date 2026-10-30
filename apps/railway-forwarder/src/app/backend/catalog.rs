use super::Backend;
use crate::domain::{Environment, Project, Service};
use std::sync::Arc;
impl Backend {
    pub fn warm_up(self: &Arc<Self>) {
        let state = self.state.lock().unwrap();
        if state.data.signed_in && !state.data.demo {
            let generation = state.generation;
            let backend = self.clone();
            self.runtime.spawn(async move {
                tokio::join!(
                    backend.clone().refresh(generation),
                    backend.refresh_account(generation)
                );
            });
        }
    }
    pub(super) async fn refresh(self: Arc<Self>, generation: u64) {
        let Some(loading) = self.loading("workspaces", generation) else {
            return;
        };
        let result = self.api.workspaces().await;
        self.mutate(|state| {
            if state.generation == generation {
                match result {
                    Ok(workspaces) => {
                        state.data.workspaces = workspaces;
                        state.data.loaded_projects.clear();
                        state.data.node_errors.remove("workspaces");
                    }
                    Err(error) => {
                        state
                            .data
                            .node_errors
                            .insert("workspaces".into(), format!("{error:#}"));
                    }
                }
            }
        });
        drop(loading);
        self.start_search();
    }
    pub(super) async fn load(self: Arc<Self>, project: Project, force: bool, generation: u64) {
        if !force
            && self
                .state
                .lock()
                .unwrap()
                .data
                .loaded_projects
                .contains(&project.id)
        {
            return;
        }
        let Some(_loading) = self.loading(&project.id, generation) else {
            return;
        };
        let _ = self.fetch_project(project, generation).await;
    }
    pub(super) async fn fetch_project(
        &self,
        project: Project,
        generation: u64,
    ) -> anyhow::Result<()> {
        let result = self.api.detail(project.clone()).await;
        self.mutate(|state| {
            if state.generation == generation {
                match &result {
                    Ok(detail) => {
                        for workspace in &mut state.data.workspaces {
                            if let Some(item) = workspace
                                .projects
                                .iter_mut()
                                .find(|item| item.id == detail.id)
                            {
                                *item = detail.clone();
                            }
                        }
                        if !state.data.loaded_projects.contains(&project.id) {
                            state.data.loaded_projects.push(project.id.clone());
                        }
                        state.data.node_errors.remove(&project.id);
                    }
                    Err(error) => {
                        state
                            .data
                            .node_errors
                            .insert(project.id.clone(), format!("{error:#}"));
                    }
                }
            }
        });
        result.map(|_| ())
    }
    pub(super) async fn scan(
        self: Arc<Self>,
        environment: Environment,
        service: Service,
        generation: u64,
    ) {
        let key = format!("{}/{}", environment.id, service.id);
        let Some(_loading) = self.loading(&key, generation) else {
            return;
        };
        let result = async {
            self.ssh.prepare(&self.api, generation).await?;
            let target = self.api.instance(&environment.id, &service.id).await?;
            self.ssh.scan(&target).await
        }
        .await;
        self.mutate(|state| {
            if state.generation == generation {
                match result {
                    Ok(mut ports) => {
                        for port in &state.data.preferences.allowed {
                            if port.service_key() == key && !ports.contains(&port.remote) {
                                ports.push(port.remote.clone());
                            }
                        }
                        ports.sort_by(|a, b| a.port.cmp(&b.port).then(a.host.cmp(&b.host)));
                        state.data.ports.insert(key.clone(), ports);
                        if !state.data.scanned.contains(&key) {
                            state.data.scanned.push(key.clone());
                        }
                        state.data.node_errors.remove(&key);
                    }
                    Err(error) => {
                        state.data.node_errors.insert(key, error.to_string());
                    }
                }
            }
        });
    }
}
