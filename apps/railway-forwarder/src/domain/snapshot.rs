use super::{
    AccountPhoto, DisplayNameLabel, LoginItemStatus, MenuLayout, Port, Preferences, Workspace,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub signed_in: bool,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(skip)]
    pub account_photo: Option<std::sync::Arc<AccountPhoto>>,
    pub busy: bool,
    pub demo: bool,
    #[serde(default)]
    pub login_item: LoginItemStatus,
    pub error: Option<String>,
    pub workspaces: Vec<Workspace>,
    pub preferences: Preferences,
    pub loading: Vec<String>,
    pub node_errors: HashMap<String, String>,
    pub ports: HashMap<String, Vec<Port>>,
    pub scanned: Vec<String>,
    pub loaded_projects: Vec<String>,
    pub menu: MenuLayout,
    pub display_names: Vec<DisplayNameLabel>,
}
