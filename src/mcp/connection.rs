//! Per-run, user-private MCP credentials. Never log or export this record.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionInfo {
    address: SocketAddr,
    bearer_token: String,
    pid: u32,
}

impl ConnectionInfo {
    pub fn url(&self) -> String {
        format!("http://{}/mcp", self.address)
    }
    pub fn bearer_token(&self) -> &str {
        &self.bearer_token
    }

    fn validate(&self) -> anyhow::Result<()> {
        if !self.address.ip().is_loopback() || self.address.port() == 0 {
            anyhow::bail!("MCP connection must use a loopback address and a bound port");
        }
        if self.bearer_token.len() != 64
            || !self
                .bearer_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            anyhow::bail!("Invalid MCP connection credential");
        }
        Ok(())
    }
}

pub struct PublishedConnection {
    path: PathBuf,
    token: String,
}

impl Drop for PublishedConnection {
    fn drop(&mut self) {
        let current = load(&self.path).ok();
        if current
            .is_some_and(|info| info.pid == std::process::id() && info.bearer_token == self.token)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn connection_path() -> PathBuf {
    crate::paths::thor_data_dir()
        .join("Mcp")
        .join("connection.json")
}

pub fn publish(
    directory: &Path,
    address: SocketAddr,
    token: [u8; 32],
) -> anyhow::Result<PublishedConnection> {
    let bearer_token: String = token.iter().map(|byte| format!("{byte:02x}")).collect();
    let info = ConnectionInfo {
        address,
        bearer_token,
        pid: std::process::id(),
    };
    info.validate()?;
    secure_directory(directory)?;
    let mut nonce = [0; 12];
    getrandom::fill(&mut nonce)
        .map_err(|error| anyhow::anyhow!("MCP connection nonce: {error}"))?;
    let suffix: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    let temporary = directory.join(format!(".connection-{suffix}.tmp"));
    let path = directory.join("connection.json");
    let result = write_record(&temporary, &info).and_then(|()| Ok(fs::rename(&temporary, &path)?));
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result?;
    Ok(PublishedConnection {
        path,
        token: info.bearer_token,
    })
}

fn secure_directory(directory: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(directory)?;
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        anyhow::bail!("MCP credential directory must be a real directory");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() } {
            anyhow::bail!("MCP credential directory has a different owner");
        }
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_record(path: &Path, info: &ConnectionInfo) -> anyhow::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec(info)?)?;
    file.sync_all()?;
    Ok(())
}

pub fn load(path: &Path) -> anyhow::Result<ConnectionInfo> {
    let file = open_private_file(path)?;
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        anyhow::bail!("MCP connection record exceeds size limit");
    }
    let info: ConnectionInfo = serde_json::from_slice(&bytes)?;
    info.validate()?;
    Ok(info)
}

fn open_private_file(path: &Path) -> anyhow::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        anyhow::bail!("MCP connection record is not a regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            anyhow::bail!("MCP connection record must be private to its owner");
        }
    }
    Ok(file)
}
