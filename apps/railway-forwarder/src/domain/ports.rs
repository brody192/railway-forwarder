use serde::{Deserialize, Serialize};
pub use ssh_forward::RemotePort as Port;

/// Present one IPv6 loopback listener when a port is bound on several addresses.
/// Stored selections and forwarding destinations remain unchanged.
pub fn preferred_ports(mut ports: Vec<Port>) -> Vec<Port> {
    let ipv6: std::collections::HashSet<_> = ports
        .iter()
        .filter(|port| port.host == "::1")
        .map(|port| port.port)
        .collect();
    ports.retain(|port| port.host == "::1" || !ipv6.contains(&port.port));
    ports.sort_by(|a, b| a.port.cmp(&b.port).then(a.host.cmp(&b.host)));
    ports.dedup();
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_port_addresses_prefer_ipv6_loopback_without_hiding_other_ports() {
        let port = |host: &str, port| Port {
            host: host.into(),
            port,
        };
        assert_eq!(
            preferred_ports(vec![
                port("127.0.0.1", 8080),
                port("::1", 8080),
                port("10.0.0.2", 8080),
                port("::1", 8080),
                port("127.0.0.1", 5432),
                port("10.0.0.2", 5432),
                port("::1", 9000),
            ]),
            vec![
                port("10.0.0.2", 5432),
                port("127.0.0.1", 5432),
                port("::1", 8080),
                port("::1", 9000)
            ]
        );
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AllowedPort {
    #[serde(rename = "workspaceID")]
    pub workspace_id: String,
    #[serde(rename = "workspaceName")]
    pub workspace_name: String,
    #[serde(rename = "projectID")]
    pub project_id: String,
    #[serde(rename = "projectName")]
    pub project_name: String,
    #[serde(rename = "environmentID")]
    pub environment_id: String,
    #[serde(rename = "environmentName")]
    pub environment_name: String,
    #[serde(rename = "serviceID")]
    pub service_id: String,
    #[serde(rename = "serviceName")]
    pub service_name: String,
    #[serde(rename = "localPort")]
    pub local_port: u16,
    pub remote: Port,
}
impl AllowedPort {
    pub fn service_key(&self) -> String {
        format!("{}/{}", self.environment_id, self.service_id)
    }
    pub fn id(&self) -> String {
        format!(
            "{}/{}/{}/{}:{}",
            self.project_id,
            self.environment_id,
            self.service_id,
            self.remote.host,
            self.remote.port
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TunnelState {
    Connecting,
    Connected,
    Failed(String),
}
