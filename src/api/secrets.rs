use std::collections::HashMap;

use parking_lot::Mutex;
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Invalid credential reference")]
    InvalidReference,
    #[error("Credential not found or no longer available")]
    Missing,
    #[error("Credential exceeds size limit")]
    TooLarge,
    #[error("Credential storage is unavailable on this platform")]
    Unavailable,
    #[error("Credential storage failed (OS status {0})")]
    Provider(i32),
    #[error("Credential random source unavailable")]
    Random,
    #[error("Credential is not UTF-8")]
    InvalidEncoding,
}

#[derive(Clone, Serialize)]
#[serde(transparent)]
pub struct SecretRef(String);

impl<'de> Deserialize<'de> for SecretRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let (kind, id) = text.split_once(':').ok_or_else(|| serde::de::Error::custom("Invalid credential reference"))?;
        if !matches!(kind, "temporary" | "keychain") || id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(serde::de::Error::custom("Invalid credential reference"));
        }
        Ok(Self(text))
    }
}

pub struct SecretValue(Zeroizing<String>);

impl SecretValue {
    pub fn expose(&self) -> &str { &self.0 }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("SecretValue([REDACTED])") }
}

/// Saved credentials live in macOS Keychain; temporary credentials are scoped
/// to this vault instance and erased when deleted or when the instance drops.
#[derive(Default)]
pub struct SecretVault { temporary: Mutex<HashMap<String, Zeroizing<String>>> }

impl SecretVault {
    pub fn store_temporary(&self, value: &str) -> Result<SecretRef, SecretError> {
        let reference = new_reference("temporary", value)?;
        self.temporary.lock().insert(reference.0.clone(), Zeroizing::new(value.into()));
        Ok(reference)
    }

    pub fn store_persistent(&self, value: &str) -> Result<SecretRef, SecretError> {
        let reference = new_reference("keychain", value)?;
        platform::store(&reference.0, value)?;
        Ok(reference)
    }

    pub fn resolve(&self, reference: &SecretRef) -> Result<SecretValue, SecretError> {
        if reference.0.starts_with("temporary:") {
            return self.temporary.lock().get(&reference.0).cloned().map(SecretValue).ok_or(SecretError::Missing);
        }
        let bytes = Zeroizing::new(platform::load(&reference.0)?);
        let value = std::str::from_utf8(&bytes).map_err(|_| SecretError::InvalidEncoding)?;
        Ok(SecretValue(Zeroizing::new(value.into())))
    }

    pub fn delete(&self, reference: &SecretRef) -> Result<(), SecretError> {
        if reference.0.starts_with("temporary:") {
            self.temporary.lock().remove(&reference.0).ok_or(SecretError::Missing)?;
            return Ok(());
        }
        platform::delete(&reference.0)
    }
}

fn new_reference(kind: &str, value: &str) -> Result<SecretRef, SecretError> {
    if value.len() > 64 * 1024 { return Err(SecretError::TooLarge); }
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| SecretError::Random)?;
    let id: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(SecretRef(format!("{kind}:{id}")))
}

#[cfg(target_os = "macos")]
mod platform {
    use super::SecretError;
    use security_framework::passwords;
    const SERVICE: &str = "com.nirmalhk7.thor.api";

    fn error(error: security_framework::base::Error) -> SecretError {
        match error.code() { -25300 => SecretError::Missing, code => SecretError::Provider(code) }
    }
    pub(super) fn store(account: &str, value: &str) -> Result<(), SecretError> {
        passwords::set_generic_password(SERVICE, account, value.as_bytes()).map_err(error)
    }
    pub(super) fn load(account: &str) -> Result<Vec<u8>, SecretError> {
        passwords::get_generic_password(SERVICE, account).map_err(error)
    }
    pub(super) fn delete(account: &str) -> Result<(), SecretError> {
        passwords::delete_generic_password(SERVICE, account).map_err(error)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::SecretError;
    pub(super) fn store(_: &str, _: &str) -> Result<(), SecretError> { Err(SecretError::Unavailable) }
    pub(super) fn load(_: &str) -> Result<Vec<u8>, SecretError> { Err(SecretError::Unavailable) }
    pub(super) fn delete(_: &str) -> Result<(), SecretError> { Err(SecretError::Unavailable) }
}
