//! Query-aware catalog loading. Search owns its requests so cancellation drops them.
use super::Backend;
use crate::domain::Workspace;
use anyhow::{Context, Result, bail};
use futures_util::{StreamExt, stream};
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use uuid::Uuid;

const SEARCH_TIMEOUT: Duration = Duration::from_secs(90);
const SEARCH_CONCURRENCY: usize = 4;

#[derive(Clone)]
pub(super) struct SearchRequest {
    id: Uuid,
    query: String,
}

pub(super) struct SearchTask {
    id: Uuid,
    handle: JoinHandle<()>,
    cancelled: bool,
}

impl Backend {
    pub(super) fn request_search(self: &Arc<Self>, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.cancel_search();
            return;
        }
        self.pause_search();
        self.mutate(|state| {
            state.search_request = Some(SearchRequest {
                id: Uuid::new_v4(),
                query: query.into(),
            });
            state.data.node_errors.remove("search");
        });
        self.start_search();
    }

    pub(super) fn pause_search(&self) {
        if let Some(task) = self.search.lock().unwrap().as_mut() {
            task.handle.abort();
            task.cancelled = true;
        }
        self.mutate(|state| {
            state.loads.remove("search");
            if let Some(request) = &mut state.search_request {
                request.id = Uuid::new_v4();
            }
        });
    }

    pub(super) fn cancel_search(&self) {
        self.pause_search();
        self.mutate(|state| {
            state.search_request = None;
            state.data.node_errors.remove("search");
        });
    }

    pub(super) fn start_search(self: &Arc<Self>) {
        let mut task = self.search.lock().unwrap();
        let state = self.state.lock().unwrap();
        let Some(request) = state.search_request.clone() else {
            return;
        };
        if state.data.demo || !state.data.signed_in || state.loads.contains_key("workspaces") {
            return;
        }
        if task.as_ref().is_some_and(|task| {
            task.id == request.id && !task.cancelled && !task.handle.is_finished()
        }) {
            return;
        }
        let generation = state.generation;
        drop(state);
        let previous = task.take();
        if let Some(previous) = &previous {
            previous.handle.abort();
        }
        let id = request.id;
        let backend = self.clone();
        let Some(loading) = self.loading("search", generation) else {
            return;
        };
        let handle = self.runtime.spawn(async move {
            let _loading = loading;
            // Wait for the previous request's loading guards and HTTP futures to drop.
            if let Some(previous) = previous {
                let _ = previous.handle.await;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            let result = backend.search_catalog(&request.query, generation).await;
            backend.mutate(|state| {
                if state.generation == generation
                    && state
                        .search_request
                        .as_ref()
                        .is_some_and(|request| request.id == id)
                {
                    match result {
                        Ok(()) => {
                            state.data.node_errors.remove("search");
                        }
                        Err(error) => {
                            state
                                .data
                                .node_errors
                                .insert("search".into(), format!("{error:#}"));
                        }
                    }
                }
            });
        });
        *task = Some(SearchTask {
            id,
            handle,
            cancelled: false,
        });
    }

    pub(super) async fn search_catalog(
        self: &Arc<Self>,
        query: &str,
        generation: u64,
    ) -> Result<()> {
        tokio::time::timeout(SEARCH_TIMEOUT, self.search_projects(query, generation))
            .await
            .context("Search timed out before all projects could be checked. Retry or narrow the search.")?
    }

    async fn search_projects(self: &Arc<Self>, query: &str, generation: u64) -> Result<()> {
        let uuid = Uuid::parse_str(query.trim()).ok().map(|id| id.to_string());
        if let Some(id) = &uuid {
            let target = cached_target(&self.state.lock().unwrap().data.workspaces, id);
            let project_id = match target {
                CachedTarget::Workspace => return Ok(()),
                CachedTarget::Project(id) => Some(id),
                CachedTarget::Unknown => self.api.project_for_uuid(id).await?,
            };
            if let Some(id) = project_id {
                let project = {
                    let state = self.state.lock().unwrap();
                    if state.data.loaded_projects.contains(&id) {
                        return Ok(());
                    }
                    state.data.workspaces.iter().flat_map(|w| &w.projects)
                        .find(|project| project.id == id).cloned()
                }.context("That UUID belongs to a project unavailable to the current account or project selection.")?;
                // A user may also be opening this project. A duplicate read is safe,
                // while skipping an in-flight load could incorrectly finish search early.
                let _loading = self.loading(&project.id, generation);
                return self.fetch_project(project, generation).await;
            }
        }

        let projects = {
            let state = self.state.lock().unwrap();
            let mut projects: Vec<_> = state
                .data
                .workspaces
                .iter()
                .flat_map(|w| &w.projects)
                .filter(|p| !state.data.loaded_projects.contains(&p.id))
                .cloned()
                .collect();
            projects.sort_by_key(|p| {
                !state
                    .data
                    .preferences
                    .allowed
                    .iter()
                    .any(|port| port.project_id == p.id)
            });
            projects
        };
        // Scoped futures, rather than detached tasks, make cancellation and the
        // deadline stop the entire batch, including requests waiting on the network.
        let mut loads = stream::iter(projects)
            .map(|project| {
                let backend = self.clone();
                async move {
                    let id = project.id.clone();
                    let _loading = backend.loading(&id, generation);
                    backend.fetch_project(project, generation).await
                }
            })
            .buffer_unordered(SEARCH_CONCURRENCY);
        let mut failures = 0;
        while let Some(result) = loads.next().await {
            failures += usize::from(result.is_err());
            if let Some(id) = &uuid {
                if cached_target(&self.state.lock().unwrap().data.workspaces, id)
                    != CachedTarget::Unknown
                {
                    return Ok(());
                }
            }
        }
        if failures > 0 {
            bail!(
                "Search could not check {failures} projects. Results may be incomplete. Retry to check them again."
            );
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum CachedTarget {
    Workspace,
    Project(String),
    Unknown,
}

fn cached_target(workspaces: &[Workspace], id: &str) -> CachedTarget {
    for workspace in workspaces {
        if workspace.id.eq_ignore_ascii_case(id) {
            return CachedTarget::Workspace;
        }
        for project in &workspace.projects {
            if project.id.eq_ignore_ascii_case(id)
                || project.environments.iter().any(|env| {
                    env.id.eq_ignore_ascii_case(id)
                        || env.services.iter().any(|service| {
                            service.id.eq_ignore_ascii_case(id)
                                || service.instance_id.eq_ignore_ascii_case(id)
                        })
                })
            {
                return CachedTarget::Project(project.id.clone());
            }
        }
    }
    CachedTarget::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Environment, Project, Service};

    #[test]
    fn replacing_and_clearing_queries_cancels_work_and_releases_loading() {
        let backend = Backend::new(true).unwrap();
        backend.runtime.block_on(async {
            let id = Uuid::new_v4();
            let loading = backend.loading("search", 0).unwrap();
            let handle = backend.runtime.spawn(async move {
                let _loading = loading;
                std::future::pending::<()>().await;
            });
            *backend.search.lock().unwrap() = Some(SearchTask {
                id,
                handle,
                cancelled: false,
            });
            backend.mutate(|state| {
                state.search_request = Some(SearchRequest {
                    id,
                    query: "old".into(),
                });
                state
                    .data
                    .node_errors
                    .insert("search".into(), "old error".into());
            });
            backend.request_search("new");
            let previous = backend.search.lock().unwrap().take().unwrap();
            assert!(previous.handle.await.unwrap_err().is_cancelled());
            assert!(!backend.snapshot().loading.contains(&"search".into()));
            assert!(!backend.snapshot().node_errors.contains_key("search"));
            assert_eq!(
                backend
                    .state
                    .lock()
                    .unwrap()
                    .search_request
                    .as_ref()
                    .unwrap()
                    .query,
                "new"
            );
            backend.pause_search();
            assert!(backend.state.lock().unwrap().search_request.is_some());
            backend.cancel_search();
            assert!(backend.state.lock().unwrap().search_request.is_none());
        });
    }

    #[test]
    fn cached_uuid_routes_only_to_its_own_project() {
        let catalog = [Workspace {
            id: "workspace".into(),
            projects: vec![
                Project {
                    id: "unrelated".into(),
                    ..Default::default()
                },
                Project {
                    id: "project".into(),
                    environments: vec![Environment {
                        id: "environment".into(),
                        services: vec![Service {
                            id: "service".into(),
                            instance_id: "instance".into(),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }];
        assert_eq!(
            cached_target(&catalog, "WORKSPACE"),
            CachedTarget::Workspace
        );
        for id in ["project", "environment", "SERVICE", "instance"] {
            assert_eq!(
                cached_target(&catalog, id),
                CachedTarget::Project("project".into())
            );
        }
        assert_eq!(cached_target(&catalog, "uncached"), CachedTarget::Unknown);
    }
}
