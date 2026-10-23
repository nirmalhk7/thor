use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{ApiError, Header, SecretRef, SecretVault, Variables};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Authentication {
    #[default]
    None,
    Bearer { token: SecretRef },
    Basic { username: String, password: SecretRef },
    ApiKeyHeader { name: String, secret: SecretRef },
    ApiKeyQuery { name: String, secret: SecretRef },
}

#[derive(Default)]
pub(crate) struct ResolvedAuth {
    pub header: Option<Header>,
    pub query: Option<Header>,
}

impl Authentication {
    pub(crate) fn resolve(&self, variables: &Variables, vault: &SecretVault) -> Result<ResolvedAuth, ApiError> {
        let header = match self {
            Self::None => return Ok(ResolvedAuth::default()),
            Self::Bearer { token } => Header::new("authorization", format!("Bearer {}", vault.resolve(token)?.expose())),
            Self::Basic { username, password } => {
                let username = variables.resolve(username)?;
                if username.contains(':') { return Err(ApiError::InvalidAuth); }
                let raw = Zeroizing::new(format!("{username}:{}", vault.resolve(password)?.expose()));
                Header::new("authorization", format!("Basic {}", STANDARD.encode(raw.as_bytes())))
            }
            Self::ApiKeyHeader { name, secret } => Header::new(variables.resolve(name)?, vault.resolve(secret)?.expose()),
            Self::ApiKeyQuery { name, secret } => return Ok(ResolvedAuth {
                header: None, query: Some(Header::new(variables.resolve(name)?, vault.resolve(secret)?.expose())),
            }),
        };
        Ok(ResolvedAuth { header: Some(header), query: None })
    }
}
