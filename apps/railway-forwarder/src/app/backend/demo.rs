use crate::domain::{AllowedPort, Environment, Port, Project, Service, Snapshot, Workspace};
pub(super) fn seed_demo(data: &mut Snapshot) {
    data.signed_in = true;
    data.email = Some("you@example.com".into());
    data.account_name = Some("Demo User".into());
    data.workspaces = vec![
        Workspace {
            id: "w1".into(),
            name: "Acme".into(),
            projects: vec![Project {
                id: "p1".into(),
                name: "Acme Platform".into(),
                environments: vec![
                    Environment {
                        id: "e1".into(),
                        name: "production".into(),
                        services: vec![
                            Service {
                                id: "s1".into(),
                                name: "Postgres".into(),
                                ..Default::default()
                            },
                            Service {
                                id: "s2".into(),
                                name: "api".into(),
                                ..Default::default()
                            },
                        ],
                    },
                    Environment {
                        id: "e2".into(),
                        name: "staging".into(),
                        services: vec![Service {
                            id: "s3".into(),
                            name: "Redis".into(),
                            ..Default::default()
                        }],
                    },
                ],
            }],
        },
        Workspace {
            id: "w2".into(),
            name: "Personal".into(),
            projects: vec![Project {
                id: "p2".into(),
                name: "Personal website".into(),
                environments: vec![Environment {
                    id: "e3".into(),
                    name: "production".into(),
                    services: vec![Service {
                        id: "s4".into(),
                        name: "web".into(),
                        ..Default::default()
                    }],
                }],
            }],
        },
    ];
    data.loaded_projects = vec!["p1".into(), "p2".into()];
    for (key, port) in [
        ("e1/s1", 5432),
        ("e1/s2", 3000),
        ("e2/s3", 6379),
        ("e3/s4", 8080),
    ] {
        data.ports.insert(
            key.into(),
            vec![Port {
                host: "127.0.0.1".into(),
                port,
            }],
        );
        data.scanned.push(key.into());
    }
    data.ports.get_mut("e1/s2").unwrap().push(Port {
        host: "::1".into(),
        port: 9090,
    });
    for workspace in &data.workspaces {
        for project in &workspace.projects {
            for environment in &project.environments {
                for service in &environment.services {
                    let port = data.ports[&format!("{}/{}", environment.id, service.id)][0].clone();
                    data.preferences.allowed.push(AllowedPort {
                        workspace_id: workspace.id.clone(),
                        workspace_name: workspace.name.clone(),
                        project_id: project.id.clone(),
                        project_name: project.name.clone(),
                        environment_id: environment.id.clone(),
                        environment_name: environment.name.clone(),
                        service_id: service.id.clone(),
                        service_name: service.name.clone(),
                        local_port: port.port,
                        remote: port,
                    });
                }
            }
        }
    }
}
