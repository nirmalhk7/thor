use super::{VaultError, VaultItemSummary, VaultLogin, VaultState, VaultStatus};
use serde_json::Value;

pub fn parse_status_json(raw: &str, has_local_session: bool) -> VaultStatus {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return VaultStatus {
                cli_present: true,
                state: if has_local_session {
                    VaultState::Unlocked
                } else {
                    VaultState::Locked
                },
                user_email: None,
                server_url: None,
            };
        }
    };
    let status = v["status"].as_str().unwrap_or("");
    let state = match status {
        "unauthenticated" => VaultState::Unauthenticated,
        "locked" => {
            if has_local_session {
                VaultState::Unlocked
            } else {
                VaultState::Locked
            }
        }
        "unlocked" => VaultState::Unlocked,
        _ => {
            if has_local_session {
                VaultState::Unlocked
            } else {
                VaultState::Locked
            }
        }
    };
    VaultStatus {
        cli_present: true,
        state,
        user_email: v["userEmail"].as_str().map(str::to_string),
        server_url: v["serverUrl"].as_str().map(str::to_string),
    }
}

pub fn parse_item_summaries(raw: &str) -> Result<Vec<VaultItemSummary>, VaultError> {
    let items: Vec<Value> =
        serde_json::from_str(raw).map_err(|e| VaultError::Parse(format!("list items: {e}")))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        // Login type == 1 in Bitwarden.
        let item_type = item["type"].as_u64().unwrap_or(0);
        if item_type != 1 {
            continue;
        }
        let id = item["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = item["name"].as_str().unwrap_or("Untitled").to_string();
        let username = item["login"]["username"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let uris = item["login"]["uris"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|u| u["uri"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.push(VaultItemSummary {
            id,
            name,
            username,
            uris,
        });
    }
    Ok(out)
}

pub fn parse_vault_login(raw: &str) -> Result<VaultLogin, VaultError> {
    let item: Value =
        serde_json::from_str(raw).map_err(|e| VaultError::Parse(format!("get item: {e}")))?;
    let id = item["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return Err(VaultError::Parse("item missing id".into()));
    }
    let login = &item["login"];
    let username = login["username"].as_str().unwrap_or("").to_string();
    let password = login["password"].as_str().unwrap_or("").to_string();
    if password.is_empty() && username.is_empty() {
        return Err(VaultError::Parse("item has no login credentials".into()));
    }
    Ok(VaultLogin {
        id,
        username,
        password,
    })
}
