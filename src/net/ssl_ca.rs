use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    PKCS_ECDSA_P256_SHA256,
};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Paths written by [`SslManager::generate_ca_and_leaf`].
#[derive(Debug, Clone)]
pub struct GeneratedCertPaths {
    pub dir: PathBuf,
    pub ca_cert_pem: PathBuf,
    pub ca_key_pem: PathBuf,
    pub leaf_cert_pem: PathBuf,
    pub leaf_key_pem: PathBuf,
}

/// Generates ECDSA P-256 SSL Certificate Authority and dynamic TLS leaf certificates.
pub struct SslManager;

const CA_TRUST_FINGERPRINT_FILE: &str = "trusted-ca.sha256";
#[cfg(target_os = "linux")]
const LINUX_CA_NICKNAME_PREFIX: &str = "Thor Dev CA ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaTrustStatus {
    Installed,
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaTrustScope {
    /// OS current-user store. This is broader than one Chromium profile.
    CurrentUser,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaTrustReadback {
    pub status: CaTrustStatus,
    pub scope: CaTrustScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaTrustAction {
    Install,
    Uninstall,
}

/// Short-lived capability proving a caller passed an explicit confirmation.
/// Fields stay private so privileged IPC must use [`CaTrustConsent::authorize`].
#[derive(Debug)]
pub struct CaTrustConsent {
    action: CaTrustAction,
    issued_at: Instant,
}

impl CaTrustConsent {
    pub const INSTALL_CONFIRMATION: &'static str = "INSTALL THOR DEV CA FOR CURRENT USER";
    pub const UNINSTALL_CONFIRMATION: &'static str = "UNINSTALL THOR DEV CA FOR CURRENT USER";

    pub fn authorize(action: CaTrustAction, confirmation: &str) -> Result<Self, String> {
        let expected = match action {
            CaTrustAction::Install => Self::INSTALL_CONFIRMATION,
            CaTrustAction::Uninstall => Self::UNINSTALL_CONFIRMATION,
        };
        if confirmation != expected {
            return Err(format!(
                "explicit CA consent required; confirmation must exactly match {expected:?}"
            ));
        }
        Ok(Self {
            action,
            issued_at: Instant::now(),
        })
    }

    fn consume(self, expected: CaTrustAction) -> Result<(), String> {
        if self.action != expected {
            return Err("CA consent capability does not authorize this action".into());
        }
        if self.issued_at.elapsed() > Duration::from_secs(120) {
            return Err("CA consent capability expired".into());
        }
        Ok(())
    }
}

impl SslManager {
    /// Thor process data directory (`$THOR_DATA_DIR` or platform `…/Thor`).
    pub fn data_dir() -> PathBuf {
        crate::paths::thor_data_dir()
    }

    /// Certificate store under the Thor data dir: `<data>/certs`.
    pub fn certs_dir() -> PathBuf {
        Self::data_dir().join("certs")
    }

    /// Generate an ECDSA P-256 CA plus a leaf cert for `*.localhost` / `localhost`.
    ///
    /// Writes PEM files under [`SslManager::certs_dir`] (or `output_dir` when provided):
    /// - `Thor-dev-ca.pem` / `Thor-dev-ca.key.pem`
    /// - `Thor-localhost.pem` / `Thor-localhost.key.pem`
    pub fn generate_ca_and_leaf() -> Result<GeneratedCertPaths, String> {
        Self::generate_ca_and_leaf_to(None)
    }

    pub fn generate_ca_and_leaf_to(
        output_dir: Option<&Path>,
    ) -> Result<GeneratedCertPaths, String> {
        let dir = output_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(Self::certs_dir);
        fs::create_dir_all(&dir).map_err(|e| format!("create certs dir: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("secure certs dir: {e}"))?;
        }

        // --- CA (ECDSA P-256) ---
        let mut ca_params =
            CertificateParams::new(Vec::<String>::new()).map_err(|e| format!("CA params: {e}"))?;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let mut ca_dn = DistinguishedName::new();
        ca_dn.push(DnType::CommonName, "Thor Dev CA");
        ca_dn.push(DnType::OrganizationName, "Thor");
        ca_params.distinguished_name = ca_dn;

        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| format!("CA keygen: {e}"))?;
        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| format!("CA self-sign: {e}"))?;

        let ca_cert_pem = dir.join("Thor-dev-ca.pem");
        let ca_key_pem = dir.join("Thor-dev-ca.key.pem");
        write_pem(&ca_cert_pem, ca_cert.pem().as_bytes(), false)
            .map_err(|e| format!("write CA cert: {e}"))?;
        write_pem(&ca_key_pem, ca_key.serialize_pem().as_bytes(), true)
            .map_err(|e| format!("write CA key: {e}"))?;

        // --- Leaf for *.localhost ---
        let mut leaf_params = CertificateParams::new(vec![
            "*.localhost".to_string(),
            "localhost".to_string(),
            "*.test".to_string(),
        ])
        .map_err(|e| format!("leaf params: {e}"))?;
        leaf_params.is_ca = IsCa::NoCa;
        leaf_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        leaf_params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        let mut leaf_dn = DistinguishedName::new();
        leaf_dn.push(DnType::CommonName, "*.localhost");
        leaf_params.distinguished_name = leaf_dn;

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| format!("leaf keygen: {e}"))?;
        // rcgen 0.13: signed_by(public_key, issuer_cert, issuer_key)
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &ca_cert, &ca_key)
            .map_err(|e| format!("leaf sign: {e}"))?;

        let leaf_cert_pem = dir.join("Thor-localhost.pem");
        let leaf_key_pem = dir.join("Thor-localhost.key.pem");
        write_pem(&leaf_cert_pem, leaf_cert.pem().as_bytes(), false)
            .map_err(|e| format!("write leaf cert: {e}"))?;
        write_pem(&leaf_key_pem, leaf_key.serialize_pem().as_bytes(), true)
            .map_err(|e| format!("write leaf key: {e}"))?;

        Ok(GeneratedCertPaths {
            dir,
            ca_cert_pem,
            ca_key_pem,
            leaf_cert_pem,
            leaf_key_pem,
        })
    }

    pub fn install_ca_trust(cert_path: &str) -> Result<(), String> {
        let _ = cert_path;
        Err("explicit CA consent required; use install_ca_trust_with_consent".into())
    }

    /// Install in the current user's trust store. This is user-scoped and is
    /// not equivalent to Chromium-profile-scoped trust.
    pub fn install_ca_trust_with_consent(
        cert_path: &str,
        consent: CaTrustConsent,
    ) -> Result<CaTrustReadback, String> {
        consent.consume(CaTrustAction::Install)?;
        let cert_path = validate_certificate_path(cert_path)?;
        let fingerprint = certificate_sha256(&cert_path)?;
        write_trust_fingerprint(&fingerprint)?;
        #[cfg(target_os = "macos")]
        {
            let login_keychain = macos_login_keychain()?;
            run_checked(
                Command::new("security").args([
                    "add-trusted-cert",
                    "-r",
                    "trustRoot",
                    "-k",
                    &login_keychain,
                    cert_path.as_os_str().to_string_lossy().as_ref(),
                ]),
                "install CA in current-user macOS login keychain",
            )?;
        }

        #[cfg(target_os = "windows")]
        {
            run_checked(
                Command::new("certutil").args([
                    "-user",
                    "-addstore",
                    "-f",
                    "Root",
                    cert_path.as_os_str().to_string_lossy().as_ref(),
                ]),
                "install CA in current-user Windows Root store",
            )?;
        }

        #[cfg(target_os = "linux")]
        {
            let nss_db = linux_nss_db()?;
            let nickname = linux_ca_nickname(&fingerprint);
            run_checked(
                Command::new("certutil").args([
                    "-d",
                    &nss_db,
                    "-A",
                    "-t",
                    "C,,",
                    "-n",
                    &nickname,
                    "-i",
                    cert_path.as_os_str().to_string_lossy().as_ref(),
                ]),
                "install CA in Linux NSS database",
            )?;
        }

        match Self::ca_trust_readback()? {
            readback @ CaTrustReadback {
                status: CaTrustStatus::Installed,
                ..
            } => Ok(readback),
            CaTrustReadback {
                status: CaTrustStatus::NotInstalled,
                ..
            } => Err(
                "CA install command succeeded but exact certificate fingerprint was not found"
                    .into(),
            ),
        }
    }

    /// Read the operating-system trust store without changing it.
    pub fn ca_trust_status() -> Result<CaTrustStatus, String> {
        Ok(Self::ca_trust_readback()?.status)
    }

    /// Read trust status plus its actual OS scope.
    pub fn ca_trust_readback() -> Result<CaTrustReadback, String> {
        let fingerprint = read_trust_fingerprint()?;
        #[cfg(target_os = "macos")]
        let output = Command::new("security")
            .args(["find-certificate", "-a", "-Z", &macos_login_keychain()?])
            .output();

        #[cfg(target_os = "windows")]
        let output = windows_find_certificate(&fingerprint);

        #[cfg(target_os = "linux")]
        let output = Command::new("certutil")
            .args([
                "-d",
                &linux_nss_db()?,
                "-L",
                "-n",
                &linux_ca_nickname(&fingerprint),
                "-a",
            ])
            .output();

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        return Err("CA trust-store readback is unsupported on this platform".into());

        let output = output.map_err(|error| format!("read CA trust status: {error}"))?;
        #[cfg(target_os = "macos")]
        let installed = output.status.success()
            && macos_fingerprint_output_contains(&output.stdout, &fingerprint);
        #[cfg(target_os = "windows")]
        let installed = output.status.success();
        #[cfg(target_os = "linux")]
        let installed = output.status.success()
            && certificate_sha256_pem(&output.stdout)
                .is_ok_and(|installed| installed == fingerprint);
        let status = if installed {
            CaTrustStatus::Installed
        } else {
            CaTrustStatus::NotInstalled
        };
        Ok(CaTrustReadback {
            status,
            scope: CaTrustScope::CurrentUser,
        })
    }

    /// Remove Thor's development CA, then confirm it is absent.
    pub fn uninstall_ca_trust() -> Result<(), String> {
        Err("explicit CA consent required; use uninstall_ca_trust_with_consent".into())
    }

    pub fn uninstall_ca_trust_with_consent(
        consent: CaTrustConsent,
    ) -> Result<CaTrustReadback, String> {
        consent.consume(CaTrustAction::Uninstall)?;
        let fingerprint = read_trust_fingerprint()?;
        if Self::ca_trust_status()? == CaTrustStatus::NotInstalled {
            return Self::ca_trust_readback();
        }

        #[cfg(target_os = "macos")]
        run_checked(
            Command::new("security").args([
                "delete-certificate",
                "-Z",
                &fingerprint,
                "-t",
                &macos_login_keychain()?,
            ]),
            "remove CA from current-user macOS login keychain",
        )?;

        #[cfg(target_os = "windows")]
        windows_remove_certificate(&fingerprint)?;

        #[cfg(target_os = "linux")]
        run_checked(
            Command::new("certutil").args([
                "-d",
                &linux_nss_db()?,
                "-D",
                "-n",
                &linux_ca_nickname(&fingerprint),
            ]),
            "remove CA from Linux NSS database",
        )?;

        match Self::ca_trust_readback()? {
            readback @ CaTrustReadback {
                status: CaTrustStatus::NotInstalled,
                ..
            } => Ok(readback),
            CaTrustReadback {
                status: CaTrustStatus::Installed,
                ..
            } => Err(
                "CA uninstall command succeeded but exact certificate fingerprint remains trusted"
                    .into(),
            ),
        }
    }
}

fn validate_certificate_path(cert_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(cert_path);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| format!("read CA certificate path {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "CA certificate must be a regular, non-symlink file: {}",
            path.display()
        ));
    }
    certificate_sha256(&path)?;
    Ok(path)
}

fn certificate_sha256(path: &Path) -> Result<String, String> {
    let pem = fs::read(path)
        .map_err(|error| format!("read CA certificate {}: {error}", path.display()))?;
    certificate_sha256_pem(&pem)
        .map_err(|error| format!("invalid PEM certificate {}: {error}", path.display()))
}

fn certificate_sha256_pem(pem: &[u8]) -> Result<String, String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let pem =
        std::str::from_utf8(pem).map_err(|error| format!("certificate is not UTF-8: {error}"))?;
    let (_, body) = pem
        .split_once(BEGIN)
        .ok_or("missing certificate begin marker")?;
    let (body, _) = body
        .split_once(END)
        .ok_or("missing certificate end marker")?;
    let encoded: String = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if encoded.is_empty() {
        return Err("certificate body is empty".into());
    }
    let der = BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| format!("decode certificate body: {error}"))?;
    let digest = Sha256::digest(der);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn trust_fingerprint_path() -> PathBuf {
    SslManager::certs_dir().join(CA_TRUST_FINGERPRINT_FILE)
}

fn write_trust_fingerprint(fingerprint: &str) -> Result<(), String> {
    validate_fingerprint(fingerprint)?;
    let dir = SslManager::certs_dir();
    fs::create_dir_all(&dir).map_err(|error| format!("create certs dir: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("secure certs dir: {error}"))?;
    }
    write_pem(&trust_fingerprint_path(), fingerprint.as_bytes(), true)
        .map_err(|error| format!("persist CA trust fingerprint: {error}"))
}

fn read_trust_fingerprint() -> Result<String, String> {
    read_trust_fingerprint_from(&trust_fingerprint_path())
}

fn read_trust_fingerprint_from(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "read CA trust fingerprint metadata {}: {error}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() || metadata.len() > 65 {
        return Err(format!(
            "CA trust fingerprint metadata must be a small regular non-symlink file: {}",
            path.display()
        ));
    }
    let fingerprint = fs::read_to_string(path)
        .map_err(|error| format!("read CA trust fingerprint {}: {error}", path.display()))?;
    let fingerprint = fingerprint.trim().to_string();
    validate_fingerprint(&fingerprint)?;
    Ok(fingerprint)
}

fn validate_fingerprint(fingerprint: &str) -> Result<(), String> {
    if fingerprint.len() != 64
        || !fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("CA trust fingerprint must be 64 lowercase hexadecimal characters".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_fingerprint_output_contains(output: &[u8], fingerprint: &str) -> bool {
    let fingerprint = fingerprint.to_ascii_uppercase();
    String::from_utf8_lossy(output).lines().any(|line| {
        line.trim()
            .strip_prefix("SHA-256 hash:")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(&fingerprint))
    })
}

#[cfg(target_os = "linux")]
fn linux_ca_nickname(fingerprint: &str) -> String {
    format!("{LINUX_CA_NICKNAME_PREFIX}{fingerprint}")
}

#[cfg(target_os = "windows")]
fn windows_find_certificate(fingerprint: &str) -> std::io::Result<std::process::Output> {
    const SCRIPT: &str = "$s=[Security.Cryptography.X509Certificates.X509Store]::new('Root','CurrentUser'); try {$s.Open('ReadOnly'); if($s.Certificates | Where-Object {$_.GetCertHashString([Security.Cryptography.HashAlgorithmName]::SHA256).Equals($env:THOR_CA_SHA256,[StringComparison]::OrdinalIgnoreCase)}) {exit 0} else {exit 1}} finally {$s.Close()}";
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("THOR_CA_SHA256", fingerprint)
        .output()
}

#[cfg(target_os = "windows")]
fn windows_remove_certificate(fingerprint: &str) -> Result<(), String> {
    const SCRIPT: &str = "$s=[Security.Cryptography.X509Certificates.X509Store]::new('Root','CurrentUser'); try {$s.Open('ReadWrite'); @($s.Certificates | Where-Object {$_.GetCertHashString([Security.Cryptography.HashAlgorithmName]::SHA256).Equals($env:THOR_CA_SHA256,[StringComparison]::OrdinalIgnoreCase)}) | ForEach-Object {$s.Remove($_)}} finally {$s.Close()}";
    run_checked(
        Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
            .env("THOR_CA_SHA256", fingerprint),
        "remove exact CA from current-user Windows Root store",
    )
}

fn write_pem(path: &Path, bytes: &[u8], private: bool) -> Result<(), String> {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(format!("refusing to replace symlink: {}", path.display()));
    }
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(if private { 0o600 } else { 0o644 })
            .custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(if private {
            0o600
        } else {
            0o644
        }))
        .map_err(|error| format!("set permissions {}: {error}", path.display()))?;
    }
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("sync {}: {error}", path.display()))
}

fn run_checked(command: &mut Command, action: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("{action}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("{action}: command exited with {}", output.status)
    } else {
        format!("{action}: command exited with {}: {detail}", output.status)
    })
}

#[cfg(target_os = "macos")]
fn macos_login_keychain() -> Result<String, String> {
    let output = Command::new("security")
        .args(["default-keychain", "-d", "user"])
        .output()
        .map_err(|error| format!("read current-user macOS login keychain: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "read current-user macOS login keychain: command exited with {}",
            output.status
        ));
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_string();
    if path.is_empty() {
        return Err("current-user macOS login keychain path is empty".into());
    }
    Ok(path)
}

#[cfg(target_os = "linux")]
fn linux_nss_db() -> Result<String, String> {
    let user_home = std::env::var_os("HOME").ok_or("HOME is unavailable for Linux NSS database")?;
    Ok(format!(
        "sql:{}",
        PathBuf::from(user_home).join(".pki/nssdb").display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_generate_ca_and_leaf_writes_pem() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("thor-ssl-test-{stamp}"));
        let paths = SslManager::generate_ca_and_leaf_to(Some(&dir)).expect("generate");

        assert!(paths.ca_cert_pem.exists());
        assert!(paths.ca_key_pem.exists());
        assert!(paths.leaf_cert_pem.exists());
        assert!(paths.leaf_key_pem.exists());

        let ca_pem = fs::read_to_string(&paths.ca_cert_pem).unwrap();
        let leaf_pem = fs::read_to_string(&paths.leaf_cert_pem).unwrap();
        assert!(ca_pem.contains("BEGIN CERTIFICATE"));
        assert!(leaf_pem.contains("BEGIN CERTIFICATE"));
        let ca_fingerprint = certificate_sha256(&paths.ca_cert_pem).unwrap();
        let leaf_fingerprint = certificate_sha256(&paths.leaf_cert_pem).unwrap();
        assert_eq!(ca_fingerprint.len(), 64);
        assert_ne!(ca_fingerprint, leaf_fingerprint);
        assert!(fs::read_to_string(&paths.ca_key_pem)
            .unwrap()
            .contains("BEGIN PRIVATE KEY"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&paths.ca_key_pem)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(&paths.leaf_key_pem)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn trust_metadata_requires_exact_sha256_fingerprint() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("thor-ssl-metadata-test-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let metadata = dir.join(CA_TRUST_FINGERPRINT_FILE);
        let fingerprint = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        write_pem(&metadata, fingerprint.as_bytes(), true).unwrap();
        assert_eq!(read_trust_fingerprint_from(&metadata).unwrap(), fingerprint);

        write_pem(&metadata, b"Thor Dev CA", true).unwrap();
        assert!(read_trust_fingerprint_from(&metadata).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = dir.join("trusted-ca-link.sha256");
            symlink(&metadata, &symlink_path).unwrap();
            assert!(read_trust_fingerprint_from(&symlink_path).is_err());
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_readback_matches_only_exact_sha256_fingerprint() {
        let expected = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let other = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
        let output = format!("SHA-256 hash: {}\n", other.to_ascii_uppercase());
        assert!(!macos_fingerprint_output_contains(
            output.as_bytes(),
            expected
        ));

        let output = format!("{output}SHA-256 hash: {}\n", expected.to_ascii_uppercase());
        assert!(macos_fingerprint_output_contains(
            output.as_bytes(),
            expected
        ));
    }

    #[test]
    fn ca_trust_requires_explicit_action_bound_consent() {
        assert!(CaTrustConsent::authorize(CaTrustAction::Install, "yes").is_err());
        assert!(CaTrustConsent::authorize(
            CaTrustAction::Install,
            CaTrustConsent::INSTALL_CONFIRMATION
        )
        .is_ok());
        let consent =
            CaTrustConsent::authorize(CaTrustAction::Install, CaTrustConsent::INSTALL_CONFIRMATION)
                .unwrap();
        assert!(consent.consume(CaTrustAction::Uninstall).is_err());
        assert!(SslManager::install_ca_trust("unused").is_err());
        assert!(SslManager::uninstall_ca_trust().is_err());
    }
}
