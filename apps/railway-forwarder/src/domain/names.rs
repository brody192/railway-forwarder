use super::{AllowedPort, Workspace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unicode_properties::UnicodeEmoji;
use unicode_segmentation::UnicodeSegmentation;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplayNameLabel {
    pub key: String,
    pub label: String,
    pub value: String,
    pub original: String,
    pub custom: bool,
}
pub fn clean_name(value: &str, fallback: &str) -> String {
    let filtered: String = value
        .graphemes(true)
        .filter(|part| {
            !part
                .chars()
                .any(|c| (c.is_emoji_char() && !c.is_ascii()) || c == '\u{20e3}')
        })
        .flat_map(str::chars)
        .filter(|c| !matches!(c, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}'))
        .collect();
    let value = filtered.trim();
    if value.is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

pub fn display_names(
    workspaces: &[Workspace],
    allowed: &[AllowedPort],
    overrides: &HashMap<String, String>,
) -> HashMap<String, DisplayNameLabel> {
    let mut names = HashMap::new();
    let mut insert = |kind: &str, id: &str, original: &str| {
        let key = format!("{}/{id}", kind.to_lowercase());
        let original = clean_name(original, kind);
        let value = clean_name(overrides.get(&key).unwrap_or(&original), kind);
        names.insert(
            key.clone(),
            DisplayNameLabel {
                key: key.clone(),
                label: kind.into(),
                value,
                original,
                custom: overrides.contains_key(&key),
            },
        );
    };
    for port in allowed {
        insert("Workspace", &port.workspace_id, &port.workspace_name);
        insert("Project", &port.project_id, &port.project_name);
        insert("Environment", &port.environment_id, &port.environment_name);
        insert("Service", &port.service_id, &port.service_name);
    }
    for workspace in workspaces {
        insert("Workspace", &workspace.id, &workspace.name);
        for project in &workspace.projects {
            insert("Project", &project.id, &project.name);
            for environment in &project.environments {
                insert("Environment", &environment.id, &environment.name);
                for service in &environment.services {
                    insert("Service", &service.id, &service.name);
                }
            }
        }
    }
    names
}

pub fn project_catalog(
    workspaces: &[Workspace],
    names: &HashMap<String, DisplayNameLabel>,
) -> Vec<Workspace> {
    let mut result = workspaces.to_vec();
    let rename = |kind: &str, id: &str| names[&format!("{kind}/{id}")].value.clone();
    for workspace in &mut result {
        workspace.name = rename("workspace", &workspace.id);
        for project in &mut workspace.projects {
            project.name = rename("project", &project.id);
            for environment in &mut project.environments {
                environment.name = rename("environment", &environment.id);
                for service in &mut environment.services {
                    service.name = rename("service", &service.id);
                }
            }
        }
    }
    result
}
