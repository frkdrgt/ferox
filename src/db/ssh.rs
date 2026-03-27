use anyhow::{anyhow, Result};
use async_trait::async_trait;
use russh::client;
use russh_keys::key::PublicKey;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::config::SshAuthMethod;

struct TunnelHandler;

#[async_trait]
impl client::Handler for TunnelHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Establishes an SSH tunnel and returns the local port on 127.0.0.1.
pub async fn establish_tunnel(
    ssh_host: &str,
    ssh_port: u16,
    ssh_user: &str,
    auth: &SshAuthMethod,
    pg_host: &str,
    pg_port: u16,
) -> Result<u16> {
    let config = Arc::new(client::Config::default());

    let mut session = client::connect(config, (ssh_host, ssh_port), TunnelHandler)
        .await
        .map_err(|e| anyhow!("SSH connect failed: {e}"))?;

    let authed = match auth {
        SshAuthMethod::Password(pw) => session
            .authenticate_password(ssh_user, pw)
            .await
            .map_err(|e| anyhow!("SSH auth failed: {e}"))?,
        SshAuthMethod::PrivateKey { path } => {
            let key = russh_keys::load_secret_key(path, None)
                .map_err(|e| anyhow!("Failed to load key {path}: {e}"))?;
            session
                .authenticate_publickey(ssh_user, Arc::new(key))
                .await
                .map_err(|e| anyhow!("SSH pubkey auth failed: {e}"))?
        }
    };

    if !authed {
        return Err(anyhow!("SSH authentication rejected"));
    }

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = listener.local_addr()?.port();

    let pg_host = pg_host.to_owned();
    tokio::spawn(async move {
        loop {
            let Ok((local_stream, _)) = listener.accept().await else {
                break;
            };
            let pg_host2 = pg_host.clone();
            let channel = match session
                .channel_open_direct_tcpip(&pg_host2, pg_port as u32, "127.0.0.1", local_port as u32)
                .await
            {
                Ok(ch) => ch,
                Err(_) => break,
            };
            tokio::spawn(forward(local_stream, channel));
        }
    });

    Ok(local_port)
}

async fn forward(
    mut local: tokio::net::TcpStream,
    mut channel: russh::Channel<client::Msg>,
) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            n = local.read(&mut buf) => {
                match n {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if channel.data(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                }
            }
            msg = channel.wait() => {
                match msg {
                    Some(russh::ChannelMsg::Data { data }) => {
                        if local.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    None | Some(russh::ChannelMsg::Eof) | Some(russh::ChannelMsg::Close) => break,
                    _ => {}
                }
            }
        }
    }
}
