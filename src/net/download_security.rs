use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Explicit verdict reported by Chromium/Safe Browsing or another native source.
/// `Unverified` is the default; Thor never infers an allow verdict.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadVerdict {
    #[default]
    Unverified,
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSecurityStatus {
    Unverified,
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DownloadQuarantineStatus {
    Unsupported {
        platform: String,
    },
    Missing,
    Present {
        value: String,
        applied_during_inspection: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DownloadScannerStatus {
    NotConfigured,
    Allowed { exit_code: i32 },
    Blocked { exit_code: i32 },
    TimedOut { timeout_ms: u64 },
    Error { message: String },
}

/// External scanner command. Thor invokes the executable directly, appending
/// the completed download path as the final argument; no shell is involved.
#[derive(Debug, Clone)]
pub struct DownloadScannerConfig {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub timeout: Duration,
    pub allowed_exit_codes: Vec<i32>,
    pub blocked_exit_codes: Vec<i32>,
}

impl DownloadScannerConfig {
    pub fn new(executable: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            executable: executable.into(),
            arguments: Vec::new(),
            timeout,
            allowed_exit_codes: vec![0],
            blocked_exit_codes: vec![1],
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadSecurityRequest {
    pub path: PathBuf,
    pub native_safe_browsing_verdict: DownloadVerdict,
    /// On macOS, preserve an existing quarantine attribute or add a minimal
    /// Thor-generated one when missing. Other platforms report Unsupported.
    pub ensure_macos_quarantine: bool,
    pub scanner: Option<DownloadScannerConfig>,
}

impl DownloadSecurityRequest {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            native_safe_browsing_verdict: DownloadVerdict::Unverified,
            ensure_macos_quarantine: true,
            scanner: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEvidence {
    pub path: PathBuf,
    pub byte_length: u64,
    pub sha256: String,
    pub inspected_at_unix_ms: u128,
    pub native_safe_browsing_verdict: DownloadVerdict,
    pub quarantine: DownloadQuarantineStatus,
    pub scanner: DownloadScannerStatus,
    pub status: DownloadSecurityStatus,
}

pub struct DownloadSecurity;

impl DownloadSecurity {
    /// Inspect a completed download. This function never opens the file as an
    /// executable, launches it, or deletes it.
    pub fn inspect_completed(request: DownloadSecurityRequest) -> Result<DownloadEvidence, String> {
        let mut verified = VerifiedDownload::open(&request.path)?;
        let sha256 = verified.sha256()?;
        let byte_length = verified.metadata.len();

        let quarantine = quarantine_status(&request.path, request.ensure_macos_quarantine)?;
        let scanner = match &request.scanner {
            Some(config) => run_scanner(config, &request.path),
            None => DownloadScannerStatus::NotConfigured,
        };
        verified.confirm_unchanged(&sha256)?;

        let status = aggregate_status(request.native_safe_browsing_verdict, &scanner);
        let inspected_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("read evidence timestamp: {error}"))?
            .as_millis();
        Ok(DownloadEvidence {
            path: request.path,
            byte_length,
            sha256,
            inspected_at_unix_ms,
            native_safe_browsing_verdict: request.native_safe_browsing_verdict,
            quarantine,
            scanner,
            status,
        })
    }
}

fn aggregate_status(
    native_verdict: DownloadVerdict,
    scanner: &DownloadScannerStatus,
) -> DownloadSecurityStatus {
    if native_verdict == DownloadVerdict::Blocked
        || matches!(scanner, DownloadScannerStatus::Blocked { .. })
    {
        return DownloadSecurityStatus::Blocked;
    }
    if native_verdict == DownloadVerdict::Allowed
        && matches!(
            scanner,
            DownloadScannerStatus::NotConfigured | DownloadScannerStatus::Allowed { .. }
        )
    {
        return DownloadSecurityStatus::Allowed;
    }
    DownloadSecurityStatus::Unverified
}

struct VerifiedDownload {
    path: PathBuf,
    file: File,
    metadata: std::fs::Metadata,
    modified: Option<SystemTime>,
}

impl VerifiedDownload {
    fn open(path: &Path) -> Result<Self, String> {
        let path_metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("read download metadata {}: {error}", path.display()))?;
        if path_metadata.file_type().is_symlink() || !path_metadata.file_type().is_file() {
            return Err(format!(
                "completed download must be a regular non-symlink file: {}",
                path.display()
            ));
        }

        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .map_err(|error| format!("open completed download {}: {error}", path.display()))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("read open download metadata {}: {error}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "opened download is not a regular file: {}",
                path.display()
            ));
        }
        if !same_file(&path_metadata, &metadata) {
            return Err(format!(
                "download changed while opening: {}",
                path.display()
            ));
        }
        let modified = metadata.modified().ok();
        Ok(Self {
            path: path.to_path_buf(),
            file,
            metadata,
            modified,
        })
    }

    fn sha256(&mut self) -> Result<String, String> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("rewind download {}: {error}", self.path.display()))?;
        let mut reader = BufReader::new(&mut self.file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|error| format!("hash download {}: {error}", self.path.display()))?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        let digest = hasher.finalize();
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    fn confirm_unchanged(&mut self, expected_sha256: &str) -> Result<(), String> {
        let path_metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            format!("re-read download metadata {}: {error}", self.path.display())
        })?;
        let open_metadata = self
            .file
            .metadata()
            .map_err(|error| format!("re-read open download {}: {error}", self.path.display()))?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.file_type().is_file()
            || !same_file(&path_metadata, &open_metadata)
            || open_metadata.len() != self.metadata.len()
            || open_metadata.modified().ok() != self.modified
        {
            return Err(format!(
                "download changed during security inspection: {}",
                self.path.display()
            ));
        }
        let actual_sha256 = self.sha256()?;
        if actual_sha256 != expected_sha256 {
            return Err(format!(
                "download contents changed during security inspection: {}",
                self.path.display()
            ));
        }
        Ok(())
    }
}

#[cfg(unix)]
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn run_scanner(config: &DownloadScannerConfig, download: &Path) -> DownloadScannerStatus {
    const MAX_SCANNER_TIMEOUT: Duration = Duration::from_secs(300);
    if config.timeout.is_zero() || config.timeout > MAX_SCANNER_TIMEOUT {
        return DownloadScannerStatus::Error {
            message: "scanner timeout must be between 1ms and 300s".into(),
        };
    }
    if config.allowed_exit_codes.is_empty()
        || config.blocked_exit_codes.is_empty()
        || config
            .allowed_exit_codes
            .iter()
            .chain(&config.blocked_exit_codes)
            .any(|code| *code < 0)
    {
        return DownloadScannerStatus::Error {
            message: "scanner requires non-empty, non-negative allow and block exit-code mappings"
                .into(),
        };
    }
    if config
        .allowed_exit_codes
        .iter()
        .any(|code| config.blocked_exit_codes.contains(code))
    {
        return DownloadScannerStatus::Error {
            message: "scanner allow and block exit-code mappings overlap".into(),
        };
    }
    if !config.executable.is_absolute() {
        return DownloadScannerStatus::Error {
            message: "scanner executable must use an absolute path".into(),
        };
    }
    let executable = match config.executable.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            return DownloadScannerStatus::Error {
                message: format!("resolve scanner executable: {error}"),
            }
        }
    };
    if !executable
        .metadata()
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
    {
        return DownloadScannerStatus::Error {
            message: "scanner executable is not a regular file".into(),
        };
    }
    if download
        .canonicalize()
        .ok()
        .is_some_and(|path| path == executable)
    {
        return DownloadScannerStatus::Error {
            message: "refusing to execute the downloaded file as its own scanner".into(),
        };
    }

    let mut command = Command::new(&executable);
    command
        .args(&config.arguments)
        .arg(download)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return DownloadScannerStatus::Error {
                message: format!("start scanner: {error}"),
            }
        }
    };
    let deadline = Instant::now() + config.timeout;
    loop {
        match child.try_wait() {
            Ok(Some(exit)) => {
                let Some(code) = exit.code() else {
                    return DownloadScannerStatus::Error {
                        message: "scanner terminated without an exit code".into(),
                    };
                };
                if config.blocked_exit_codes.contains(&code) {
                    return DownloadScannerStatus::Blocked { exit_code: code };
                }
                if config.allowed_exit_codes.contains(&code) {
                    return DownloadScannerStatus::Allowed { exit_code: code };
                }
                return DownloadScannerStatus::Error {
                    message: format!("scanner exited with unrecognized status {code}"),
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                if let Err(error) = child.kill() {
                    return DownloadScannerStatus::Error {
                        message: format!("scanner timed out and termination failed: {error}"),
                    };
                }
                let _ = child.wait();
                return DownloadScannerStatus::TimedOut {
                    timeout_ms: config.timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                };
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return DownloadScannerStatus::Error {
                    message: format!("read scanner exit status: {error}"),
                };
            }
        }
    }
}

#[cfg(target_os = "macos")]
const QUARANTINE_ATTRIBUTE: &[u8] = b"com.apple.quarantine\0";

#[cfg(target_os = "macos")]
fn quarantine_status(
    path: &Path,
    ensure_present: bool,
) -> Result<DownloadQuarantineStatus, String> {
    match read_quarantine_attribute(path)? {
        Some(bytes) => Ok(DownloadQuarantineStatus::Present {
            value: String::from_utf8_lossy(&bytes).into_owned(),
            applied_during_inspection: false,
        }),
        None if !ensure_present => Ok(DownloadQuarantineStatus::Missing),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("read quarantine timestamp: {error}"))?
                .as_secs();
            let value = format!("0081;{timestamp:x};Thor;");
            write_quarantine_attribute(path, value.as_bytes())?;
            let readback = read_quarantine_attribute(path)?
                .ok_or("quarantine attribute write succeeded but readback was missing")?;
            if readback != value.as_bytes() {
                return Err("quarantine attribute readback did not match written value".into());
            }
            Ok(DownloadQuarantineStatus::Present {
                value,
                applied_during_inspection: true,
            })
        }
    }
}

#[cfg(target_os = "macos")]
fn xattr_path(path: &Path) -> Result<std::ffi::CString, String> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("download path contains NUL: {}", path.display()))
}

#[cfg(target_os = "macos")]
fn read_quarantine_attribute(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let path = xattr_path(path)?;
    // SAFETY: both C strings are NUL-terminated and live across the calls. The
    // first call requests size only; the second writes into an allocated buffer.
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            QUARANTINE_ATTRIBUTE.as_ptr().cast(),
            std::ptr::null_mut(),
            0,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if size < 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOATTR) {
            return Ok(None);
        }
        return Err(format!("read macOS quarantine attribute size: {error}"));
    }
    let size = usize::try_from(size).map_err(|_| "invalid quarantine attribute size")?;
    if size > 4096 {
        return Err(format!("quarantine attribute exceeds 4096 bytes ({size})"));
    }
    let mut bytes = vec![0u8; size];
    // SAFETY: the buffer has exactly `size` writable bytes and pointers remain
    // valid for the duration of the syscall.
    let read = unsafe {
        libc::getxattr(
            path.as_ptr(),
            QUARANTINE_ATTRIBUTE.as_ptr().cast(),
            bytes.as_mut_ptr().cast(),
            size,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if read < 0 {
        return Err(format!(
            "read macOS quarantine attribute: {}",
            std::io::Error::last_os_error()
        ));
    }
    bytes.truncate(usize::try_from(read).map_err(|_| "invalid quarantine read length")?);
    Ok(Some(bytes))
}

#[cfg(target_os = "macos")]
fn write_quarantine_attribute(path: &Path, value: &[u8]) -> Result<(), String> {
    let path = xattr_path(path)?;
    // SAFETY: path/name/value pointers are valid for the syscall. NOFOLLOW
    // prevents a late symlink substitution from redirecting metadata writes.
    let result = unsafe {
        libc::setxattr(
            path.as_ptr(),
            QUARANTINE_ATTRIBUTE.as_ptr().cast(),
            value.as_ptr().cast(),
            value.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if result != 0 {
        return Err(format!(
            "write macOS quarantine attribute: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn quarantine_status(
    _path: &Path,
    _ensure_present: bool,
) -> Result<DownloadQuarantineStatus, String> {
    Ok(DownloadQuarantineStatus::Unsupported {
        platform: std::env::consts::OS.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "thor-download-security-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn regular_file_hash_and_unverified_verdict_are_explicit() {
        let dir = test_dir("hash");
        let path = dir.join("download.bin");
        fs::write(&path, b"hello").unwrap();
        let mut request = DownloadSecurityRequest::new(&path);
        request.ensure_macos_quarantine = false;
        let evidence = DownloadSecurity::inspect_completed(request).unwrap();
        assert_eq!(evidence.byte_length, 5);
        assert_eq!(
            evidence.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(evidence.status, DownloadSecurityStatus::Unverified);
        assert_eq!(
            evidence.native_safe_browsing_verdict,
            DownloadVerdict::Unverified
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_download_is_rejected() {
        use std::os::unix::fs::symlink;
        let dir = test_dir("symlink");
        let target = dir.join("target.bin");
        let link = dir.join("download.bin");
        fs::write(&target, b"payload").unwrap();
        symlink(&target, &link).unwrap();
        let error =
            DownloadSecurity::inspect_completed(DownloadSecurityRequest::new(&link)).unwrap_err();
        assert!(error.contains("non-symlink"));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    fn fake_scanner(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn configured_scanner_exit_codes_are_checked() {
        let dir = test_dir("scanner");
        let download = dir.join("download.bin");
        fs::write(&download, b"payload").unwrap();

        let allowed = fake_scanner(&dir, "allow-scanner", "exit 0");
        let mut request = DownloadSecurityRequest::new(&download);
        request.ensure_macos_quarantine = false;
        request.native_safe_browsing_verdict = DownloadVerdict::Allowed;
        request.scanner = Some(DownloadScannerConfig::new(allowed, Duration::from_secs(1)));
        let evidence = DownloadSecurity::inspect_completed(request).unwrap();
        assert_eq!(
            evidence.scanner,
            DownloadScannerStatus::Allowed { exit_code: 0 }
        );
        assert_eq!(evidence.status, DownloadSecurityStatus::Allowed);

        let blocked = fake_scanner(&dir, "block-scanner", "exit 1");
        let mut request = DownloadSecurityRequest::new(&download);
        request.ensure_macos_quarantine = false;
        request.native_safe_browsing_verdict = DownloadVerdict::Allowed;
        request.scanner = Some(DownloadScannerConfig::new(blocked, Duration::from_secs(1)));
        let evidence = DownloadSecurity::inspect_completed(request).unwrap();
        assert_eq!(
            evidence.scanner,
            DownloadScannerStatus::Blocked { exit_code: 1 }
        );
        assert_eq!(evidence.status, DownloadSecurityStatus::Blocked);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn scanner_content_mutation_is_rejected_even_when_size_and_mtime_match() {
        let dir = test_dir("scanner-mutation");
        let download = dir.join("download.bin");
        fs::write(&download, b"payload").unwrap();
        let scanner = fake_scanner(
            &dir,
            "mutating-scanner",
            "touch -r \"$1\" \"$1.stamp\"\nprintf mutated > \"$1\"\ntouch -r \"$1.stamp\" \"$1\"\nexit 0",
        );
        let mut request = DownloadSecurityRequest::new(&download);
        request.ensure_macos_quarantine = false;
        request.native_safe_browsing_verdict = DownloadVerdict::Allowed;
        request.scanner = Some(DownloadScannerConfig::new(scanner, Duration::from_secs(1)));

        let error = DownloadSecurity::inspect_completed(request).unwrap_err();
        assert!(error.contains("contents changed during security inspection"));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn configured_scanner_timeout_is_bounded_and_unverified() {
        let dir = test_dir("timeout");
        let download = dir.join("download.bin");
        fs::write(&download, b"payload").unwrap();
        let scanner = fake_scanner(&dir, "slow-scanner", "while :; do :; done");
        let mut request = DownloadSecurityRequest::new(&download);
        request.ensure_macos_quarantine = false;
        request.native_safe_browsing_verdict = DownloadVerdict::Allowed;
        request.scanner = Some(DownloadScannerConfig::new(
            scanner,
            Duration::from_millis(30),
        ));
        let evidence = DownloadSecurity::inspect_completed(request).unwrap();
        assert!(matches!(
            evidence.scanner,
            DownloadScannerStatus::TimedOut { .. }
        ));
        assert_eq!(evidence.status, DownloadSecurityStatus::Unverified);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn overlapping_scanner_exit_mappings_fail_private_without_execution() {
        let dir = test_dir("overlap");
        let download = dir.join("download.bin");
        fs::write(&download, b"payload").unwrap();
        let marker = dir.join("scanner-ran");
        let scanner = fake_scanner(
            &dir,
            "overlap-scanner",
            &format!("touch {:?}\nexit 0", marker),
        );
        let mut config = DownloadScannerConfig::new(scanner, Duration::from_secs(1));
        config.blocked_exit_codes = vec![0];
        let mut request = DownloadSecurityRequest::new(&download);
        request.ensure_macos_quarantine = false;
        request.native_safe_browsing_verdict = DownloadVerdict::Allowed;
        request.scanner = Some(config);
        let evidence = DownloadSecurity::inspect_completed(request).unwrap();
        assert!(matches!(
            evidence.scanner,
            DownloadScannerStatus::Error { .. }
        ));
        assert_eq!(evidence.status, DownloadSecurityStatus::Unverified);
        assert!(!marker.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_quarantine_write_is_read_back() {
        let dir = test_dir("quarantine");
        let path = dir.join("download.bin");
        fs::write(&path, b"payload").unwrap();
        let evidence =
            DownloadSecurity::inspect_completed(DownloadSecurityRequest::new(&path)).unwrap();
        assert!(matches!(
            evidence.quarantine,
            DownloadQuarantineStatus::Present { .. }
        ));
        let _ = fs::remove_dir_all(dir);
    }
}
