use std::net::SocketAddr;
use std::path::PathBuf;
use thor::mcp::connection::{load, publish};

fn directory() -> PathBuf {
    let mut bytes = [0; 12];
    getrandom::fill(&mut bytes).unwrap();
    let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    std::env::temp_dir().join(format!("thor-mcp-test-{suffix}"))
}

#[test]
fn connection_credentials_are_private_and_removed_when_server_stops() {
    let directory = directory();
    let guard = publish(&directory, "127.0.0.1:8765".parse().unwrap(), [7; 32]).unwrap();
    let path = directory.join("connection.json");
    let connection = load(&path).unwrap();
    assert_eq!(connection.url(), "http://127.0.0.1:8765/mcp");
    assert_eq!(connection.bearer_token(), "07".repeat(32));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(guard);
    assert!(!path.exists());
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn old_server_cannot_remove_replacement_connection_credentials() {
    let directory = directory();
    let address: SocketAddr = "127.0.0.1:8765".parse().unwrap();
    let old = publish(&directory, address, [1; 32]).unwrap();
    let current = publish(&directory, address, [2; 32]).unwrap();
    drop(old);
    assert_eq!(
        load(&directory.join("connection.json"))
            .unwrap()
            .bearer_token(),
        "02".repeat(32)
    );
    drop(current);
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn connections_cannot_send_browser_credentials_to_public_servers() {
    let directory = directory();
    assert!(publish(&directory, "192.0.2.10:8765".parse().unwrap(), [1; 32]).is_err());
    assert!(!directory.exists());
}
