# ssh-forward

A Rust SDK for in-process SSH port forwarding and Linux listener discovery, built on `russh` and `russh-sftp`. It has no dependency on Railway, GPUI, AppKit, the SSH executable or a shell. The credential file implementation targets Unix platforms.

Use it from another application with a path dependency:

```toml
[dependencies]
ssh-forward = { path = "/path/to/railway forwarder/crates/ssh-forward" }
```

```rust,no_run
use ssh_forward::{Client, Config, RemotePort};

# async fn example() -> anyhow::Result<()> {
let client = Client::new(Config::new(
    "ssh.example.com",
    "state/id_ed25519",
    "state/known_hosts",
));
// Register this key with the SSH server before connecting.
let public_key = client.public_key()?;
let mut tunnel = client.forward(
    "alice",
    "127.0.0.1:8080".parse()?,
    RemotePort::new("127.0.0.1", 3000),
).await?;
println!("Listening on {}", tunnel.local_addr());
tunnel.wait().await?;
# Ok(()) }
```

The caller supplies a running Tokio runtime and handles authentication with its own provider. `Client` accepts an SSH endpoint, identity path and known-hosts path. `Config` also lets you set the SSH port, key comment and connection timeout. An existing unencrypted OpenSSH identity is preserved; otherwise an Ed25519 key is generated. Private state uses owner-only files and directories, atomic writes and symlink rejection. The first server key is pinned automatically. Changed host keys are rejected.

`forward` binds and authenticates before returning an owned `Tunnel`. Bind to port `0` to let the OS choose a local port. Each incoming connection gets its own SSH direct TCP channel, with up to 256 simultaneous connections per tunnel. The SDK accepts the bind address you supply; use a loopback address to restrict local access.

Keep the `Tunnel` alive while forwarding. `stop()` requests shutdown, `close().await` waits for cleanup, and dropping the handle cancels forwarding and releases its sockets. `wait()` returns when the tunnel stops or the SSH transport fails. Canceling a `wait()` future leaves the handle usable. Tunnels do not reconnect automatically.

`discover_ports(username)` reads `/proc/net/tcp` and `/proc/net/tcp6` over SFTP, returning TCP listeners as `RemotePort` values. It needs a Linux server exposing those files through SFTP. It never executes commands. `probe(username, &remote)` checks authentication and channel opening without application traffic. Neither operation requires a forwarding listener. The optional `serde` feature adds serialization for `RemotePort`.

From the workspace root:

```sh
cargo test -p ssh-forward
cargo run -p ssh-forward --example forward -- \
  ssh.example.com alice 127.0.0.1:8080 127.0.0.1 3000 ./ssh-state
```

The example prints the public key before connecting. Register that key and rerun it if your server does not already authorize the identity. Tests run a local SSH/SFTP server and cover public API forwarding, discovery, concurrent transfers, half-close, cancellation, socket cleanup and changed-key rejection.
