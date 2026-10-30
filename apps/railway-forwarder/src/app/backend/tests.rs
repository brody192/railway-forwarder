use super::*;
use serde_json::{Value, json};

#[test]
fn sign_out_preserves_saved_ports_and_menu_preferences_across_restarts() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = Scratch(std::env::temp_dir().join(format!("rf-sign-out-{}", Uuid::new_v4())));
    let files = Files {
        app: scratch.0.join("settings"),
        credentials: scratch.0.join("credentials"),
    };
    let mut data = Snapshot::default();
    seed_demo(&mut data);
    data.preferences.client_id = "test-client".into();
    data.preferences.allowed[0].local_port = 15432;
    data.preferences
        .menu
        .names
        .insert("project/p1".into(), "Custom project".into());
    data.preferences.menu.section_order = vec!["w2/p2".into(), "w1/p1".into()];
    let first = data.preferences.allowed[0].clone();
    data.preferences.menu.port_order = vec![first.id()];
    let preferences = serde_json::to_value(&data.preferences).unwrap();
    persistence::write(&files.settings(), &data.preferences).unwrap();
    let tokens =
        json!({"accessToken":"test-access", "refreshToken":"test-refresh", "expiresAt":900000000.});
    persistence::write(&files.credentials.join("tokens.json"), &tokens).unwrap();
    let backend = Backend::with_files(false, files.clone()).unwrap();
    assert!(backend.snapshot().signed_in);
    backend.mutate(|state| {
        state.data.email = Some("you@example.com".into());
        state.data.workspaces = data.workspaces;
    });
    assert!(backend.tunnels.begin(&first).unwrap().is_some());
    backend.tunnel_state(&first.id(), Some(TunnelState::Connected));
    backend.dispatch(json!({"action":"signOut"}));
    backend.runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while backend.snapshot().busy || Arc::strong_count(&backend) != 1 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    });
    let signed_out = backend.snapshot();
    assert!(!signed_out.signed_in);
    assert!(signed_out.email.is_none());
    assert!(signed_out.menu.sections.is_empty());
    assert!(backend.state.lock().unwrap().tunnels.is_empty());
    assert!(!files.credentials.join("tokens.json").exists());
    assert_eq!(
        serde_json::to_value(&signed_out.preferences).unwrap(),
        preferences
    );
    assert_eq!(
        persistence::read::<Value>(&files.settings())
            .unwrap()
            .unwrap(),
        preferences
    );
    // A disconnected reservation must no longer block the saved local port.
    assert!(backend.tunnels.begin(&first).unwrap().is_some());
    backend.stop_all();
    drop(backend);
    let signed_out_restart = Backend::with_files(false, files.clone()).unwrap();
    assert!(!signed_out_restart.snapshot().signed_in);
    assert_eq!(
        serde_json::to_value(&signed_out_restart.snapshot().preferences).unwrap(),
        preferences
    );
    drop(signed_out_restart);
    // Restore a test session to exercise startup after signing in, without
    // opening OAuth or touching the real account's credential files.
    persistence::write(&files.credentials.join("tokens.json"), &tokens).unwrap();
    let signed_in_restart = Backend::with_files(false, files).unwrap();
    let restored = signed_in_restart.snapshot();
    assert!(restored.signed_in);
    assert_eq!(
        serde_json::to_value(&restored.preferences).unwrap(),
        preferences
    );
    assert_eq!(restored.menu.sections[0].id, "w2/p2");
    let project = restored
        .menu
        .sections
        .iter()
        .find(|section| section.id == "w1/p1")
        .unwrap();
    assert_eq!(project.project_name, "Custom project");
    assert_eq!(project.ports[0].port.local_port, 15432);
    assert!(
        restored
            .menu
            .sections
            .iter()
            .flat_map(|section| &section.ports)
            .all(|port| !port.is_on)
    );
}

#[test]
fn port_commands_preserve_allowlist_and_validate_local_ports() {
    let backend = Backend::new(true).unwrap();
    let first = backend.snapshot().preferences.allowed[0].clone();
    backend.dispatch(json!({"action":"localPort","id":first.id(),"value":"15432"}));
    backend.dispatch(json!({"action":"localPort","id":first.id(),"value":"70000"}));
    let data = backend.snapshot();
    assert!(data.error.is_some());
    assert_eq!(data.preferences.allowed[0].local_port, 15432);
    backend.dispatch(json!({"action":"dismissError"}));
    let mut command = json!({"action":"allowPort","workspaceID":"w1","projectID":"p1","environmentID":"e1","serviceID":"s1","port":{"host":"127.0.0.1","port":5432},"enabled":false});
    backend.dispatch(command.clone());
    assert!(
        !backend
            .snapshot()
            .preferences
            .allowed
            .iter()
            .any(|p| p.id() == first.id())
    );
    command["port"]["port"] = json!(65000);
    command["enabled"] = json!(true);
    backend.dispatch(command.clone());
    assert!(backend.snapshot().error.is_some());
    assert_eq!(backend.snapshot().preferences.allowed.len(), 3);
    backend.dispatch(json!({"action":"addManual","workspaceID":"w1","projectID":"p1","environmentID":"e1","serviceID":"s1","value":"65000"}));
    backend.dispatch(command);
    assert_eq!(backend.snapshot().preferences.allowed.len(), 4);
    let encoded = serde_json::to_string(&backend.snapshot()).unwrap();
    assert!(!encoded.contains("accessToken") && !encoded.contains("refreshToken"));
}

#[test]
fn uuid_names_project_globally_and_reset_to_live_catalog() {
    let backend = Backend::new(true).unwrap();
    backend.mutate(|state| {
        let project = &mut state.data.workspaces[0].projects[0];
        let shared = project.environments[0].services[1].clone();
        project.environments[1].services.push(shared);
        let mut shared_port = state.data.preferences.allowed[1].clone();
        shared_port.environment_id = "e2".into();
        shared_port.remote.port = 4000;
        shared_port.local_port = 4000;
        state.data.preferences.allowed.push(shared_port);
    });
    for (key, value) in [
        ("workspace/w1", " 🛠️ My  Workspace "),
        ("project/p1", "My Project"),
        ("environment/e1", "Live"),
        ("service/s2", "Gateway"),
        ("project/p2", "My Project"),
    ] {
        backend.dispatch(json!({"action":"displayName","id":key,"value":value}));
        assert!(backend.snapshot().error.is_none());
    }
    let data = backend.snapshot();
    assert_eq!(data.workspaces[0].name, "My  Workspace");
    let project = &data.workspaces[0].projects[0];
    assert_eq!(project.name, "My Project");
    assert_eq!(project.environments[0].name, "Live");
    let shared: Vec<_> = project
        .environments
        .iter()
        .flat_map(|env| &env.services)
        .filter(|service| service.id == "s2")
        .collect();
    assert_eq!(shared.len(), 2);
    assert!(shared.iter().all(|service| service.name == "Gateway"));
    let section = data
        .menu
        .sections
        .iter()
        .find(|section| section.id == "w1/p1")
        .unwrap();
    assert_eq!(section.workspace_name, "My  Workspace");
    let rows: Vec<_> = section
        .ports
        .iter()
        .filter(|port| port.port.service_id == "s2")
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|port| port.title.starts_with("Gateway ·")));
    assert_eq!(
        rows.iter()
            .find(|p| p.port.environment_id == "e1")
            .unwrap()
            .context,
        "My Project › Live"
    );
    assert_eq!(data.preferences.allowed[1].service_name, "api");
    backend
        .mutate(|state| state.data.workspaces[0].projects[0].name = "Updated Railway Name".into());
    backend.dispatch(json!({"action":"displayName","id":"project/p1","value":""}));
    let data = backend.snapshot();
    assert_eq!(data.workspaces[0].projects[0].name, "Updated Railway Name");
    assert_eq!(data.workspaces[1].projects[0].name, "My Project");
    assert_eq!(
        data.menu
            .sections
            .iter()
            .find(|s| s.id == "w1/p1")
            .unwrap()
            .project_name,
        "Updated Railway Name"
    );
}

#[test]
fn menu_editing_and_persistence_keep_fixed_labels_and_port_selections() {
    let backend = Backend::new(true).unwrap();
    let initial = backend.snapshot();
    let section = &initial.menu.sections[0];
    let a = &section.ports[0].id;
    let b = &section.ports[1].id;
    for key in [
        "header".into(),
        "disconnect".into(),
        "settings".into(),
        "quit".into(),
        format!("port/{a}"),
    ] {
        backend.dispatch(json!({"action":"displayName","id":key,"value":"Changed"}));
        assert!(backend.snapshot().error.is_some());
        backend.dispatch(json!({"action":"dismissError"}));
    }
    backend.dispatch(json!({"action":"menuMove","kind":"port","id":a,"targetID":b}));
    assert_eq!(&backend.snapshot().menu.sections[0].ports[0].id, b);
    backend.dispatch(json!({"action":"menuMove","kind":"section","id":section.id,"targetID":initial.menu.sections[1].id}));
    assert_eq!(
        backend.snapshot().menu.sections.last().unwrap().id,
        section.id
    );
    let data = backend.snapshot();
    let mut saved = serde_json::to_value(&data.preferences).unwrap();
    let restored: Preferences = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(restored.menu.port_order, data.preferences.menu.port_order);
    saved.as_object_mut().unwrap().remove("menu");
    let without_menu: Preferences = serde_json::from_value(saved).unwrap();
    assert_eq!(without_menu.allowed.len(), 4);
    assert!(without_menu.menu.names.is_empty());
    backend.dispatch(json!({"action":"menuReset"}));
    let data = backend.snapshot();
    assert_eq!(data.preferences.allowed.len(), 4);
    assert!(data.preferences.menu.port_order.is_empty());
    assert_eq!(
        (
            data.menu.header.as_str(),
            data.menu.disconnect.as_str(),
            data.menu.settings.as_str(),
            data.menu.quit.as_str()
        ),
        ("Railway Forwarder", "Disconnect", "Settings", "Quit")
    );
}
