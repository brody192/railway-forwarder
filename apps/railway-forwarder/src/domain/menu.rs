use super::{AllowedPort, DisplayNameLabel, MenuPreferences, TunnelState, clean_name};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MenuLayout {
    pub header: String,
    pub disconnect: String,
    pub settings: String,
    pub quit: String,
    pub sections: Vec<MenuSection>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuSection {
    pub id: String,
    pub project_name: String,
    pub workspace_name: String,
    pub ports: Vec<MenuPort>,
    pub name_keys: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPort {
    pub id: String,
    #[serde(rename = "sectionID")]
    pub section_id: String,
    pub title: String,
    pub context: String,
    pub port: AllowedPort,
    pub is_on: bool,
    pub state: Option<String>,
    pub error: Option<String>,
    pub name_keys: Vec<String>,
}
pub fn menu_layout(
    ports: &[AllowedPort],
    preferences: &MenuPreferences,
    names: &HashMap<String, DisplayNameLabel>,
    states: &HashMap<String, TunnelState>,
) -> MenuLayout {
    let name = |kind: &str, id: &str| {
        names
            .get(&format!("{kind}/{id}"))
            .map(|name| name.value.clone())
            .unwrap_or_default()
    };
    let rank =
        |ids: &[String], id: &str| ids.iter().position(|item| item == id).unwrap_or(usize::MAX);
    let mut sorted = ports.to_vec();
    sorted.sort_by_key(|port| {
        (
            clean_name(&port.workspace_name, "Workspace"),
            port.workspace_id.clone(),
            clean_name(&port.project_name, "Project"),
            port.project_id.clone(),
            port.environment_name.clone(),
            port.service_name.clone(),
            port.remote.port,
            port.id(),
        )
    });
    let mut sections: Vec<MenuSection> = Vec::new();
    for port in sorted {
        let section_id = format!("{}/{}", port.workspace_id, port.project_id);
        if sections
            .last()
            .is_none_or(|section| section.id != section_id)
        {
            sections.push(MenuSection {
                id: section_id.clone(),
                project_name: name("project", &port.project_id),
                workspace_name: name("workspace", &port.workspace_id),
                ports: Vec::new(),
                name_keys: vec![
                    format!("project/{}", port.project_id),
                    format!("workspace/{}", port.workspace_id),
                ],
            });
        }
        let mapping = if port.local_port == port.remote.port {
            port.remote.port.to_string()
        } else {
            format!("{} → {}", port.local_port, port.remote.port)
        };
        let state = states.get(&port.id());
        let (status, error) = match state {
            Some(TunnelState::Connecting) => (Some("connecting".into()), None),
            Some(TunnelState::Connected) => (Some("connected".into()), None),
            Some(TunnelState::Failed(message)) => (Some("failed".into()), Some(message.clone())),
            None => (None, None),
        };
        let row = MenuPort {
            id: port.id(),
            section_id,
            title: format!("{} · {mapping}", name("service", &port.service_id)),
            context: format!(
                "{} › {}",
                name("project", &port.project_id),
                name("environment", &port.environment_id)
            ),
            name_keys: vec![
                format!("service/{}", port.service_id),
                format!("workspace/{}", port.workspace_id),
                format!("project/{}", port.project_id),
                format!("environment/{}", port.environment_id),
            ],
            is_on: matches!(
                state,
                Some(TunnelState::Connecting | TunnelState::Connected)
            ),
            state: status,
            error,
            port,
        };
        sections.last_mut().unwrap().ports.push(row);
    }
    for section in &mut sections {
        section
            .ports
            .sort_by_key(|port| rank(&preferences.port_order, &port.id));
    }
    sections.sort_by_key(|section| rank(&preferences.section_order, &section.id));
    MenuLayout {
        header: "Railway Forwarder".into(),
        disconnect: "Disconnect".into(),
        settings: "Settings".into(),
        quit: "Quit".into(),
        sections,
    }
}
