//! Keeps the displayed search tree stable while the user interacts with it.
use crate::domain::{Project, Workspace};
use std::collections::HashSet;

#[derive(Default)]
pub(super) struct SearchResults {
    displayed: Vec<Workspace>,
    pinned: Option<Vec<Workspace>>,
    pending_projects: HashSet<String>,
}

impl SearchResults {
    pub(super) fn clear(&mut self) {
        self.pinned = None;
        self.pending_projects.clear();
    }

    pub(super) fn pin(&mut self) {
        self.pinned.get_or_insert_with(|| self.displayed.clone());
    }

    pub(super) fn display(
        &mut self,
        latest: impl FnOnce() -> Vec<Workspace>,
        names: &[Workspace],
    ) -> Vec<Workspace> {
        let mut visible = self.pinned.clone().unwrap_or_else(latest);
        sync_names(&mut visible, names);
        self.displayed = visible.clone();
        visible
    }

    pub(super) fn open_branch(
        &mut self,
        id: &str,
        latest: &[Workspace],
        loaded_projects: &[String],
    ) {
        let Some(pinned) = &mut self.pinned else {
            return;
        };
        if let Some(id) = id.strip_prefix("w/") {
            if let (Some(target), Some(source)) = (
                pinned.iter_mut().find(|workspace| workspace.id == id),
                latest.iter().find(|workspace| workspace.id == id),
            ) {
                *target = source.clone();
            }
        } else if let Some(id) = id.strip_prefix("p/") {
            replace_project(pinned, latest, id);
            if !loaded_projects.iter().any(|loaded| loaded == id) {
                self.pending_projects.insert(id.into());
            }
        } else if let Some(path) = id.strip_prefix("e/") {
            if let Some((project_id, environment_id)) = path.split_once('/') {
                let target = pinned
                    .iter_mut()
                    .flat_map(|workspace| &mut workspace.projects)
                    .find(|project| project.id == project_id);
                let source = latest
                    .iter()
                    .flat_map(|workspace| &workspace.projects)
                    .find(|project| project.id == project_id);
                if let (Some(target), Some(source)) = (target, source) {
                    if let (Some(target), Some(source)) = (
                        target
                            .environments
                            .iter_mut()
                            .find(|env| env.id == environment_id),
                        source
                            .environments
                            .iter()
                            .find(|env| env.id == environment_id),
                    ) {
                        *target = source.clone();
                    }
                }
            }
        }
    }

    pub(super) fn close_branch(&mut self, id: &str) {
        if let Some(project_id) = id.strip_prefix("p/") {
            self.pending_projects.remove(project_id);
        }
    }

    pub(super) fn receive_requested_projects(&mut self, latest: &[Workspace], loaded: &[String]) {
        let Some(pinned) = &mut self.pinned else {
            return;
        };
        self.pending_projects.retain(|id| {
            if loaded.contains(id) {
                replace_project(pinned, latest, id);
                false
            } else {
                true
            }
        });
    }
}

fn replace_project(target: &mut [Workspace], source: &[Workspace], id: &str) {
    let source: Option<&Project> = source
        .iter()
        .flat_map(|workspace| &workspace.projects)
        .find(|project| project.id == id);
    let target = target
        .iter_mut()
        .flat_map(|workspace| &mut workspace.projects)
        .find(|project| project.id == id);
    if let (Some(target), Some(source)) = (target, source) {
        *target = source.clone();
    }
}

fn sync_names(visible: &mut [Workspace], names: &[Workspace]) {
    for workspace in visible {
        let Some(source) = names.iter().find(|source| source.id == workspace.id) else {
            continue;
        };
        workspace.name.clone_from(&source.name);
        for project in &mut workspace.projects {
            let Some(source) = source
                .projects
                .iter()
                .find(|source| source.id == project.id)
            else {
                continue;
            };
            project.name.clone_from(&source.name);
            for environment in &mut project.environments {
                let Some(source) = source
                    .environments
                    .iter()
                    .find(|source| source.id == environment.id)
                else {
                    continue;
                };
                environment.name.clone_from(&source.name);
                for service in &mut environment.services {
                    if let Some(source) = source
                        .services
                        .iter()
                        .find(|source| source.id == service.id)
                    {
                        service.name.clone_from(&source.name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Environment;

    fn catalog() -> Vec<Workspace> {
        vec![Workspace {
            id: "w".into(),
            name: "Workspace".into(),
            projects: vec![
                Project {
                    id: "a".into(),
                    name: "First".into(),
                    ..Default::default()
                },
                Project {
                    id: "b".into(),
                    name: "Second".into(),
                    ..Default::default()
                },
            ],
        }]
    }

    #[test]
    fn background_matches_and_children_cannot_move_pinned_rows_but_names_stay_live() {
        let initial = catalog();
        let mut results = SearchResults::default();
        results.display(|| initial.clone(), &initial);
        results.pin();
        let mut later = initial.clone();
        later[0].projects[0].name = "Renamed".into();
        later[0].projects[0].environments.push(Environment {
            id: "e".into(),
            ..Default::default()
        });
        later.insert(
            0,
            Workspace {
                id: "new-workspace".into(),
                ..Default::default()
            },
        );
        let visible = results.display(|| later.clone(), &later);
        assert_eq!(visible.len(), 1);
        assert_eq!(
            visible[0]
                .projects
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(visible[0].projects[0].environments.is_empty());
        assert_eq!(visible[0].projects[0].name, "Renamed");
        results.clear();
        assert_eq!(results.display(|| later.clone(), &later).len(), 2);
    }

    #[test]
    fn explicit_project_open_can_receive_its_children_without_updating_other_branches() {
        let initial = catalog();
        let mut results = SearchResults::default();
        results.display(|| initial.clone(), &initial);
        results.pin();
        results.open_branch("p/a", &initial, &[]);
        let mut loaded = initial.clone();
        for project in &mut loaded[0].projects {
            project.environments.push(Environment {
                id: format!("e-{}", project.id),
                ..Default::default()
            });
        }
        results.receive_requested_projects(&loaded, &["a".into(), "b".into()]);
        let visible = results.display(|| loaded.clone(), &loaded);
        assert_eq!(visible[0].projects[0].environments.len(), 1);
        assert!(visible[0].projects[1].environments.is_empty());

        results.clear();
        results.display(|| initial.clone(), &initial);
        results.pin();
        results.open_branch("p/a", &initial, &[]);
        results.close_branch("p/a");
        results.receive_requested_projects(&loaded, &["a".into()]);
        assert!(
            results.display(|| loaded.clone(), &loaded)[0].projects[0]
                .environments
                .is_empty()
        );
    }
}
