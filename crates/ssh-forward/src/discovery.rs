use crate::{RemotePort, client::Connection};
use anyhow::{Context, Result, bail};
use std::{collections::HashSet, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};
const MAX_TABLE_BYTES: u64 = 4 * 1024 * 1024;
pub(crate) async fn discover(connection: Connection) -> Result<Vec<RemotePort>> {
    let result = timeout(Duration::from_secs(25), async {
        let channel = connection.handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let sftp = russh_sftp::client::SftpSession::new(channel.into_stream()).await?;
        let mut listeners = String::new();
        let mut readable = false;
        let mut failures = Vec::new();
        for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let table = async {
                let mut file = sftp.open(path).await?;
                let mut bytes = Vec::new();
                (&mut file)
                    .take(MAX_TABLE_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .await?;
                file.shutdown().await?;
                if bytes.len() as u64 > MAX_TABLE_BYTES {
                    bail!("The TCP socket table is too large.");
                }
                Ok::<_, anyhow::Error>(String::from_utf8(bytes)?)
            }
            .await;
            match table {
                Ok(table) => {
                    readable = true;
                    listeners.push_str(&listener_rows(path, &table));
                }
                Err(error) => failures.push(error.context(path.to_owned())),
            }
        }
        let _ = sftp.close().await;
        if !readable {
            return Err(failures
                .into_iter()
                .next()
                .context("No remote TCP socket tables were available.")?);
        }
        Ok::<_, anyhow::Error>(parse_ports(&listeners))
    })
    .await
    .context("SFTP port discovery timed out.")
    .and_then(|result| result);
    connection.close().await;
    result.map_err(discovery_error)
}

fn discovery_error(error: anyhow::Error) -> anyhow::Error {
    let timed_out = error.chain().any(|cause| {
        cause.is::<tokio::time::error::Elapsed>()
            || matches!(
                cause.downcast_ref::<russh_sftp::client::error::Error>(),
                Some(russh_sftp::client::error::Error::Timeout)
            )
            || cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::TimedOut)
    });
    error.context(if timed_out {
        "Port discovery timed out. Try Rescan or add the port manually."
    } else {
        "Could not discover listening ports over SFTP. Try Rescan or add the port manually."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_errors_separate_display_messages_from_diagnostic_causes() {
        let timeout = discovery_error(
            anyhow::Error::new(russh_sftp::client::error::Error::Timeout).context("/proc/net/tcp"),
        );
        assert_eq!(
            timeout.to_string(),
            "Port discovery timed out. Try Rescan or add the port manually."
        );
        assert!(
            timeout
                .downcast_ref::<russh_sftp::client::error::Error>()
                .is_some()
        );
        assert!(format!("{timeout:#}").contains("/proc/net/tcp: Timeout"));
        let unreadable =
            discovery_error(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into());
        assert_eq!(
            unreadable.to_string(),
            "Could not discover listening ports over SFTP. Try Rescan or add the port manually."
        );
        assert!(unreadable.downcast_ref::<std::io::Error>().is_some());
    }
}
pub(crate) fn listener_rows(path: &str, table: &str) -> String {
    table
        .lines()
        .filter_map(|line| {
            let columns: Vec<_> = line.split_whitespace().collect();
            (columns.len() >= 4 && columns[3] == "0A").then(|| format!("{path} {}\n", columns[1]))
        })
        .collect()
}
pub(crate) fn parse_ports(output: &str) -> Vec<RemotePort> {
    let mut ports = HashSet::new();
    for line in output.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }
        let Some((hex, port)) = parts[1].split_once(':') else {
            continue;
        };
        let Ok(port) = u16::from_str_radix(port, 16) else {
            continue;
        };
        if port == 0 {
            continue;
        }
        let host = if parts[0] == "/proc/net/tcp" && hex.len() == 8 {
            let Ok(address) = u32::from_str_radix(hex, 16) else {
                continue;
            };
            if address == 0 {
                "127.0.0.1".into()
            } else {
                std::net::Ipv4Addr::from(address.to_le_bytes()).to_string()
            }
        } else if parts[0] == "/proc/net/tcp6" && hex.len() == 32 && hex.is_ascii() {
            let words: Result<Vec<_>, _> = (0..4)
                .map(|word| u32::from_str_radix(&hex[word * 8..word * 8 + 8], 16))
                .collect();
            let Ok(words) = words else {
                continue;
            };
            let bytes: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
            if bytes.iter().all(|byte| *byte == 0) {
                "::1".into()
            } else {
                let address: [u8; 16] = bytes.try_into().expect("Four IPv6 words");
                std::net::Ipv6Addr::from(address).to_string()
            }
        } else {
            continue;
        };
        ports.insert(RemotePort { host, port });
    }
    let mut ports: Vec<_> = ports.into_iter().collect();
    ports.sort_by(|a, b| a.port.cmp(&b.port).then(a.host.cmp(&b.host)));
    ports
}
