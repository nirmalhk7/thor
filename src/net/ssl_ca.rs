use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    pub fn generate_ca_and_leaf_to(output_dir: Option<&Path>) -> Result<GeneratedCertPaths, String> {
        let dir = output_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(Self::certs_dir);
        fs::create_dir_all(&dir).map_err(|e| format!("create certs dir: {e}"))?;

        // --- CA (ECDSA P-256) ---
        let mut ca_params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| format!("CA params: {e}"))?;
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
        fs::write(&ca_cert_pem, ca_cert.pem()).map_err(|e| format!("write CA cert: {e}"))?;
        fs::write(&ca_key_pem, ca_key.serialize_pem())
            .map_err(|e| format!("write CA key: {e}"))?;

        // --- Leaf for *.localhost ---
        let mut leaf_params = CertificateParams::new(vec![
            "*.localhost".to_string(),
            "localhost".to_string(),
            "*.test".to_string(),
            "dev.local".to_string(),
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
        fs::write(&leaf_cert_pem, leaf_cert.pem())
            .map_err(|e| format!("write leaf cert: {e}"))?;
        fs::write(&leaf_key_pem, leaf_key.serialize_pem())
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
        #[cfg(target_os = "macos")]
        {
            // Register with macOS Keychain
            Command::new("security")
                .args([
                    "add-trusted-cert",
                    "-d",
                    "-r",
                    "trustRoot",
                    "-k",
                    "/Library/Keychains/System.keychain",
                    cert_path,
                ])
                .status()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "windows")]
        {
            // Register with Windows Cert Store
            Command::new("certutil")
                .args(&["-addstore", "-f", "Root", cert_path])
                .status()
                .map_err(|e| e.to_string())?;
        }

        #[cfg(target_os = "linux")]
        {
            // Register with Linux NSS DB
            Command::new("certutil")
                .args(&[
                    "-d",
                    "sql:$HOME/.pki/nssdb",
                    "-A",
                    "-t",
                    "C,,",
                    "-n",
                    "Thor Dev CA",
                    "-i",
                    cert_path,
                ])
                .status()
                .map_err(|e| e.to_string())?;
        }

        let _ = cert_path;
        Ok(())
    }
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
        assert!(fs::read_to_string(&paths.ca_key_pem)
            .unwrap()
            .contains("BEGIN PRIVATE KEY"));

        let _ = fs::remove_dir_all(&dir);
    }
}
