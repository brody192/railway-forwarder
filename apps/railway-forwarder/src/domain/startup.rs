use serde::{Deserialize, Serialize};

/// The operating system owns login-item registration and approval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginItemStatus {
    #[default]
    NotRegistered,
    Enabled,
    RequiresApproval,
    NotFound,
    Unavailable,
}

impl LoginItemStatus {
    pub fn is_registered(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}
