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
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPreferences {
    pub names: HashMap<String, String>,
    pub section_order: Vec<String>,
    pub port_order: Vec<String>,
}
