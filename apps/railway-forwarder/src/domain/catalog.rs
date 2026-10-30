use crate::domain::{AllowedPort, Workspace};
use std::collections::HashSet;

pub fn selected_paths(ports: &[AllowedPort]) -> HashSet<String> {
    ports
        .iter()
        .flat_map(|port| {
            [
                format!("w/{}", port.workspace_id),
                format!("p/{}", port.project_id),
                format!("e/{}/{}", port.project_id, port.environment_id),
                format!("s/{}/{}", port.environment_id, port.service_id),
            ]
        })
        .collect()
}

pub fn selected(workspaces: &[Workspace], ports: &[AllowedPort]) -> Vec<Workspace> {
    let paths = selected_paths(ports);
    workspaces
        .iter()
        .filter(|workspace| paths.contains(&format!("w/{}", workspace.id)))
        .cloned()
        .map(|mut workspace| {
            workspace
                .projects
                .retain(|project| paths.contains(&format!("p/{}", project.id)));
            for project in &mut workspace.projects {
                project.environments.retain(|environment| {
                    paths.contains(&format!("e/{}/{}", project.id, environment.id))
                });
                for environment in &mut project.environments {
                    environment.services.retain(|service| {
                        paths.contains(&format!("s/{}/{}", environment.id, service.id))
                    });
                }
            }
            workspace
        })
        .collect()
}

pub fn filtered(workspaces: &[Workspace], query: &str) -> Vec<Workspace> {
    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return workspaces.to_vec();
    }
    let matches = |path: &str| {
        let path = path.to_lowercase();
        words.iter().all(|word| path.contains(word))
    };
    workspaces
        .iter()
        .filter_map(|workspace| {
            let mut workspace = workspace.clone();
            let workspace_path = format!("{} {}", workspace.name, workspace.id);
            if matches(&workspace_path) {
                return Some(workspace);
            }
            workspace.projects = workspace
                .projects
                .into_iter()
                .filter_map(|mut project| {
                    let path = format!("{workspace_path} {} {}", project.name, project.id);
                    if matches(&path) {
                        return Some(project);
                    }
                    project.environments = project
                        .environments
                        .into_iter()
                        .filter_map(|mut environment| {
                            let path = format!("{path} {} {}", environment.name, environment.id);
                            if matches(&path) {
                                return Some(environment);
                            }
                            environment.services.retain(|service| {
                                matches(&format!(
                                    "{path} {} {} {}",
                                    service.name, service.id, service.instance_id
                                ))
                            });
                            (!environment.services.is_empty()).then_some(environment)
                        })
                        .collect();
                    (!project.environments.is_empty()).then_some(project)
                })
                .collect();
            (!workspace.projects.is_empty()).then_some(workspace)
        })
        .collect()
}

pub fn search_paths(workspaces: &[Workspace]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for workspace in workspaces {
        paths.insert(format!("w/{}", workspace.id));
        for project in &workspace.projects {
            if project.environments.is_empty() {
                continue;
            }
            paths.insert(format!("p/{}", project.id));
            for environment in &project.environments {
                if !environment.services.is_empty() {
                    paths.insert(format!("e/{}/{}", project.id, environment.id));
                }
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Environment, Port, Project, Service};

    #[test]
    fn service_search_keeps_matching_ancestors_and_excludes_siblings() {
        let catalog = vec![Workspace {
            id: "w".into(),
            name: "Acme".into(),
            projects: vec![Project {
                id: "p".into(),
                name: "Platform".into(),
                environments: vec![Environment {
                    id: "e".into(),
                    name: "production".into(),
                    services: vec![
                        Service {
                            id: "s1".into(),
                            name: "Postgres DB".into(),
                            ..Default::default()
                        },
                        Service {
                            id: "s2".into(),
                            name: "api".into(),
                            ..Default::default()
                        },
                    ],
                }],
            }],
        }];
        let results = filtered(&catalog, "production POSTGRES");
        assert_eq!(results[0].projects[0].environments[0].services.len(), 1);
        assert_eq!(results[0].projects[0].environments[0].services[0].id, "s1");
        assert!(filtered(&catalog, "missing").is_empty());
        assert_eq!(
            filtered(&catalog, "Acme")[0].projects[0].environments[0]
                .services
                .len(),
            2
        );
    }

    #[test]
    fn uuid_search_matches_each_level_and_instance_without_unrelated_siblings() {
        let workspace = "a1251db6-bbef-48df-b902-b06e8eab12db";
        let project = "b277cf05-abcf-4425-9c76-f846ba2113c7";
        let environment = "c38fdfda-7702-4191-ad22-763e5b8cedd5";
        let service = "d47a16ad-4ca4-43a9-b122-fdb1cf91a8fe";
        let instance = "e563fdc2-a6ca-48cc-b48f-bec6f902394d";
        let catalog = vec![
            Workspace {
                id: workspace.into(),
                name: "Acme".into(),
                projects: vec![
                    Project {
                        id: project.into(),
                        name: "Platform".into(),
                        environments: vec![
                            Environment {
                                id: environment.into(),
                                name: "production".into(),
                                services: vec![
                                    Service {
                                        id: service.into(),
                                        name: "Database".into(),
                                        instance_id: "00000000-0000-4000-8000-000000000001".into(),
                                    },
                                    Service {
                                        id: "00000000-0000-4000-8000-000000000002".into(),
                                        name: "API".into(),
                                        instance_id: "00000000-0000-4000-8000-000000000003".into(),
                                    },
                                ],
                            },
                            Environment {
                                id: "00000000-0000-4000-8000-000000000004".into(),
                                name: "staging".into(),
                                services: vec![Service {
                                    id: service.into(),
                                    name: "Database".into(),
                                    instance_id: instance.into(),
                                }],
                            },
                        ],
                    },
                    Project {
                        id: "00000000-0000-4000-8000-000000000005".into(),
                        name: "Other".into(),
                        ..Default::default()
                    },
                ],
            },
            Workspace {
                id: "00000000-0000-4000-8000-000000000006".into(),
                name: "Personal".into(),
                ..Default::default()
            },
        ];
        for (query, projects, environments, services) in [
            (workspace.to_owned(), 2, 2, 3),
            (project.to_owned(), 1, 2, 3),
            (environment.to_owned(), 1, 1, 2),
            (service.to_owned(), 1, 2, 2),
            (instance.to_owned(), 1, 1, 1),
            (service.to_uppercase(), 1, 2, 2),
            (service[..8].to_owned(), 1, 2, 2),
            (format!("production {}", &service[..8]), 1, 1, 1),
        ] {
            let found = filtered(&catalog, &query);
            assert_eq!(found.len(), 1, "{query}");
            assert_eq!(found[0].id, workspace);
            assert_eq!(found[0].projects.len(), projects, "{query}");
            assert_eq!(
                found[0]
                    .projects
                    .iter()
                    .map(|project| project.environments.len())
                    .sum::<usize>(),
                environments,
                "{query}"
            );
            assert_eq!(
                found[0]
                    .projects
                    .iter()
                    .flat_map(|project| &project.environments)
                    .map(|env| env.services.len())
                    .sum::<usize>(),
                services,
                "{query}"
            );
        }
        let found = filtered(&catalog, instance);
        assert_eq!(found[0].projects[0].environments[0].name, "staging");
        assert!(filtered(&catalog, "ffffffff-ffff-4fff-8fff-ffffffffffff").is_empty());
    }
    #[test]
    fn selected_paths_expand_only_services_with_saved_ports() {
        let ports = vec![AllowedPort {
            workspace_id: "w".into(),
            project_id: "p".into(),
            environment_id: "e".into(),
            service_id: "s".into(),
            local_port: 5432,
            remote: Port {
                host: "127.0.0.1".into(),
                port: 5432,
            },
            ..Default::default()
        }];
        assert_eq!(
            selected_paths(&ports),
            HashSet::from(["w/w".into(), "p/p".into(), "e/p/e".into(), "s/e/s".into()])
        );
    }
    #[test]
    fn selected_filter_hides_siblings_and_keeps_pending_project_ancestors() {
        let backend = crate::app::backend::Backend::new(true).unwrap();
        let data = backend.snapshot();
        let selection = &data.preferences.allowed[..1];
        let result = selected(&data.workspaces, selection);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, selection[0].workspace_id);
        assert_eq!(result[0].projects.len(), 1);
        let project = &result[0].projects[0];
        assert_eq!(project.environments.len(), 1);
        assert_eq!(project.environments[0].services.len(), 1);
        assert_eq!(
            project.environments[0].services[0].id,
            selection[0].service_id
        );
        assert!(filtered(&result, "Redis").is_empty());
        assert_eq!(filtered(&result, "Postgres").len(), 1);
        assert!(selected(&data.workspaces, &[]).is_empty());
        let mut summaries = data.workspaces.clone();
        summaries[0].projects[0].environments.clear();
        let pending = selected(&summaries, selection);
        assert_eq!(pending[0].projects.len(), 1);
        assert!(pending[0].projects[0].environments.is_empty());
        // Filtering never removes ports from the saved allowlist or catalog.
        assert_eq!(backend.snapshot().preferences.allowed.len(), 4);
        assert_eq!(backend.snapshot().workspaces.len(), 2);
    }
}
