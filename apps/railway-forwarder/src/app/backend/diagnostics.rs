use super::Backend;
use anyhow::{Result, bail};
impl Backend {
    pub fn check_search(query: &str) -> Result<()> {
        let backend = Self::new(false)?;
        backend.runtime.block_on(async {
            let started = std::time::Instant::now();
            let workspaces = backend.api.workspaces().await?;
            backend.mutate(|state| state.data.workspaces = workspaces);
            backend.search_catalog(query, 0).await?;
            let data = backend.snapshot();
            let matches = crate::domain::catalog::filtered(&data.workspaces, query);
            let services = matches.iter().flat_map(|w| &w.projects)
                .flat_map(|p| &p.environments).map(|e| e.services.len()).sum::<usize>();
            println!("Search completed in {:.2}s, loaded {} project details, found {services} service matches.", started.elapsed().as_secs_f64(), data.loaded_projects.len());
            if matches.is_empty() { bail!("No matching resources."); }
            Ok(())
        })
    }
    pub fn check_ssh() -> Result<()> {
        let backend = Self::new(false)?;
        backend.runtime.block_on(async {
            let ports = backend.snapshot().preferences.allowed;
            if ports.is_empty() {
                bail!("Select a port in settings before running the SSH check.");
            }
            backend.ssh.prepare(&backend.api, 0).await?;
            for port in ports {
                let target = backend
                    .api
                    .instance(&port.environment_id, &port.service_id)
                    .await?;
                let discovered = backend.ssh.scan(&target).await?;
                backend.ssh.probe(&target, &port.remote).await?;
                println!(
                    "Verified native SSH, SFTP ({} listeners), and TCP forwarding to port {}.",
                    discovered.len(),
                    port.remote.port
                );
            }
            Ok(())
        })
    }
    pub fn check_catalog() -> Result<()> {
        let backend = Self::new(false)?;
        backend.runtime.block_on(async {
            backend.api.oauth.profile().await?;
            let workspaces = backend.api.workspaces().await?;
            let selected: std::collections::HashSet<_> = backend
                .state
                .lock()
                .unwrap()
                .data
                .preferences
                .allowed
                .iter()
                .map(|port| port.project_id.clone())
                .collect();
            let mut details = 0;
            for project in workspaces
                .iter()
                .flat_map(|workspace| &workspace.projects)
                .filter(|project| selected.contains(&project.id))
            {
                backend.api.detail(project.clone()).await?;
                details += 1;
            }
            println!(
                "Verified account email, {} workspaces, {} projects, and {details} saved project details.",
                workspaces.len(),
                workspaces
                    .iter()
                    .map(|workspace| workspace.projects.len())
                    .sum::<usize>()
            );
            Ok(())
        })
    }
}
