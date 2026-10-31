use super::AllowedPort;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(rename = "clientID")]
    pub client_id: String,
    pub allowed: Vec<AllowedPort>,
    #[serde(default)]
    pub menu: MenuPreferences,
    #[serde(default, rename = "restoreTunnelsOnLaunch")]
    pub restore_tunnels_on_launch: bool,
    #[serde(default, rename = "activeTunnels")]
    pub active_tunnels: Vec<String>,
}
impl Preferences {
    pub fn tunnels_to_restore(&self) -> Vec<AllowedPort> {
        if !self.restore_tunnels_on_launch {
            return Vec::new();
        }
        let mut seen = std::collections::HashSet::new();
        self.allowed
            .iter()
            .filter(|port| {
                let id = port.id();
                self.active_tunnels.contains(&id) && seen.insert(id)
            })
            .cloned()
            .collect()
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPreferences {
    pub names: HashMap<String, String>,
    pub section_order: Vec<String>,
    pub port_order: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Port;
    use serde_json::json;

    #[test]
    fn legacy_preferences_default_to_no_restore_and_only_remembered_selected_ports_restore() {
        let mut preferences: Preferences =
            serde_json::from_value(json!({"clientID":"", "allowed":[]})).unwrap();
        assert!(!preferences.restore_tunnels_on_launch);
        assert!(preferences.active_tunnels.is_empty());
        let port = AllowedPort {
            local_port: 15432,
            remote: Port {
                host: "127.0.0.1".into(),
                port: 5432,
            },
            ..Default::default()
        };
        let mut other = port.clone();
        other.service_id = "other".into();
        preferences.allowed = vec![port.clone(), other, port.clone()];
        preferences.active_tunnels = vec![port.id(), "removed-port".into(), port.id()];
        assert!(preferences.tunnels_to_restore().is_empty());
        preferences.restore_tunnels_on_launch = true;
        let encoded = serde_json::to_value(&preferences).unwrap();
        assert_eq!(encoded["restoreTunnelsOnLaunch"], true);
        let restored: Preferences = serde_json::from_value(encoded).unwrap();
        let ports = restored.tunnels_to_restore();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].id(), port.id());
        assert_eq!(ports[0].local_port, 15432);
    }
}
