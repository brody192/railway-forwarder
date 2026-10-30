use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub projects: Vec<Project>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub environments: Vec<Environment>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub services: Vec<Service>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    #[serde(rename = "instanceID", default)]
    pub instance_id: String,
}
