use anyhow::{Context, Result, bail};
use ssh_forward::{Client, Config, RemotePort};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 6 {
        bail!(
            "Usage: forward <ssh-host> <username> <local-address:port> <remote-host> <remote-port> <state-directory>"
        );
    }
    let state = PathBuf::from(&arguments[5]);
    let client = Client::new(Config::new(
        &arguments[0],
        state.join("id_ed25519"),
        state.join("known_hosts"),
    ));
    println!(
        "Register this public key with your SSH server:\n{}",
        client.public_key()?
    );
    let destination = RemotePort::new(
        &arguments[3],
        arguments[4].parse().context("Invalid remote port")?,
    );
    let mut tunnel = client
        .forward(
            &arguments[1],
            arguments[2].parse().context("Invalid local address")?,
            destination,
        )
        .await?;
    println!(
        "Listening on {}. Press Ctrl+C to exit.",
        tunnel.local_addr()
    );
    tunnel.wait().await
}
