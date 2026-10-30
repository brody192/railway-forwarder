//! In-process SSH forwarding with authenticated channels, host-key verification and SFTP discovery.
//!
//! No SSH executable, shell or application framework is required.
//!
//! ```no_run
//! # async fn example() -> anyhow::Result<()> {
//! use ssh_forward::{Client, Config, RemotePort};
//! let client = Client::new(Config::new("ssh.example.com", "state/id_ed25519", "state/known_hosts"));
//! // Register client.public_key()? on the SSH server before connecting.
//! let mut tunnel = client.forward("alice", "127.0.0.1:8080".parse()?, RemotePort::new("127.0.0.1", 3000)).await?;
//! println!("Listening on {}", tunnel.local_addr());
//! tunnel.wait().await?;
//! # Ok(()) }
//! ```

mod client;
mod config;
mod credentials;
mod discovery;
mod files;
mod tunnel;

pub use client::Client;
pub use config::{Config, RemotePort};
pub use tunnel::Tunnel;

#[cfg(test)]
mod tests;
