use super::{
    auth::{self, OAuth},
    graphql,
};
use crate::domain::{Environment, Project, Service, Workspace};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct Railway {
    pub oauth: Arc<OAuth>,
}
impl Railway {
    async fn query(&self, operation: &str, variables: Value) -> Result<Value> {
        let body = graphql::process(operation, &variables.to_string(), false)
            .map_err(anyhow::Error::msg)?;
        let token = self.oauth.access_token().await?;
        let response = auth::send(
            self.oauth
                .http
                .post("https://backboard.railway.com/graphql/v2")
                .bearer_auth(token)
                .json(&body),
        )
        .await?;
        graphql::process(operation, &response.text().await?, true).map_err(anyhow::Error::msg)
    }
    pub async fn workspaces(&self) -> Result<Vec<Workspace>> {
        #[derive(Deserialize)]
        struct Summary {
            id: String,
            name: String,
            projects: Vec<Named>,
        }
        #[derive(Deserialize)]
        struct Response {
            #[serde(rename = "externalWorkspaces")]
            workspaces: Vec<Summary>,
        }
        let response: Response =
            serde_json::from_value(self.query("Workspaces", json!({})).await?)?;
        let mut result: Vec<_> = response
            .workspaces
            .into_iter()
            .map(|workspace| {
                let mut projects: Vec<_> = workspace
                    .projects
                    .into_iter()
                    .map(|project| Project {
                        id: project.id,
                        name: project.name,
                        ..Default::default()
                    })
                    .collect();
                projects.sort_by_key(|project| project.name.to_lowercase());
                Workspace {
                    id: workspace.id,
                    name: workspace.name,
                    projects,
                }
            })
            .collect();
        result.sort_by_key(|workspace| workspace.name.to_lowercase());
        Ok(result)
    }
    pub async fn detail(&self, mut project: Project) -> Result<Project> {
        #[derive(Deserialize)]
        struct Instance {
            id: String,
            #[serde(rename = "serviceId")]
            service_id: String,
        }
        #[derive(Deserialize)]
        struct Env {
            id: String,
            name: String,
            #[serde(rename = "serviceInstances")]
            instances: Connection<Instance>,
        }
        #[derive(Deserialize)]
        struct Detail {
            environments: Connection<Env>,
            services: Connection<Named>,
        }
        #[derive(Deserialize)]
        struct Response {
            project: Detail,
        }
        let response: Response = serde_json::from_value(
            self.query("ProjectDetail", json!({"id":project.id}))
                .await?,
        )?;
        let names: std::collections::HashMap<_, _> = response
            .project
            .services
            .edges
            .into_iter()
            .map(|edge| (edge.node.id, edge.node.name))
            .collect();
        project.environments = response
            .project
            .environments
            .edges
            .into_iter()
            .map(|edge| {
                let env = edge.node;
                let mut services: Vec<_> = env
                    .instances
                    .edges
                    .into_iter()
                    .map(|edge| Service {
                        id: edge.node.service_id.clone(),
                        name: names
                            .get(&edge.node.service_id)
                            .cloned()
                            .unwrap_or_else(|| "Service".into()),
                        instance_id: edge.node.id,
                    })
                    .collect();
                services.sort_by_key(|service| service.name.clone());
                Environment {
                    id: env.id,
                    name: env.name,
                    services,
                }
            })
            .collect();
        project.environments.sort_by_key(|env| env.name.clone());
        Ok(project)
    }
    pub async fn instance(&self, environment: &str, service: &str) -> Result<String> {
        self.query(
            "ServiceInstance",
            json!({"environmentId":environment,"serviceId":service}),
        )
        .await?["serviceInstance"]["id"]
            .as_str()
            .map(str::to_string)
            .context("Railway returned no service instance.")
    }

    /// Resolves full service/environment IDs without listing every project's metadata.
    pub async fn project_for_uuid(&self, id: &str) -> Result<Option<String>> {
        for (operation, field) in [
            ("ServiceProject", "service"),
            ("EnvironmentProject", "environment"),
        ] {
            match self.query(operation, json!({"id":id})).await {
                Ok(data) => {
                    return data[field]["projectId"]
                        .as_str()
                        .map(|id| Some(id.to_owned()))
                        .context("Railway returned no owning project.");
                }
                Err(error) if lookup_miss(&error) => {}
                Err(error) => return Err(error).context("Could not look up that UUID."),
            }
        }
        Ok(None)
    }
    pub async fn register_key(&self, key: &str) -> Result<()> {
        let response = self.query("SshPublicKeys", json!({})).await?;
        let material = |key: &str| key.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
        if response["sshPublicKeys"]["edges"]
            .as_array()
            .context("Railway returned no SSH keys.")?
            .iter()
            .any(|edge| {
                edge["node"]["publicKey"]
                    .as_str()
                    .is_some_and(|saved| material(saved) == material(key))
            })
        {
            return Ok(());
        }
        self.query(
            "CreateSshPublicKey",
            json!({"input":{"name":"Railway Forwarder (Mac)","publicKey":key}}),
        )
        .await?;
        Ok(())
    }
}
fn lookup_miss(error: &anyhow::Error) -> bool {
    let message = error
        .to_string()
        .to_lowercase()
        .replace([' ', '_', '-'], "");
    [
        "notfound",
        "notauthorized",
        "notauthorised",
        "unauthorized",
        "forbidden",
        "accessdenied",
    ]
    .iter()
    .any(|expected| message.contains(expected))
}
#[derive(Deserialize)]
struct Named {
    id: String,
    name: String,
}
#[derive(Deserialize)]
struct Connection<T> {
    edges: Vec<Edge<T>>,
}
#[derive(Deserialize)]
struct Edge<T> {
    node: T,
}
