//! app-state: app-level data only (projects, sessions, settings).
//! Process state is NEVER here — it is derived from `.specify/` artifacts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root: String,
    pub created_unix_ms: u64,
    pub last_opened_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub provider_id: String,
    /// Reference into the OS keychain — never the secret itself.
    pub credential_ref: String,
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Option<String>,
    pub approval_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_account_holds_ref_not_secret() {
        let acc = ProviderAccount {
            provider_id: "anthropic".into(),
            credential_ref: "keychain:aix/anthropic".into(),
            default_model: Some("claude-sonnet-4-5".into()),
        };
        let json = serde_json::to_string(&acc).unwrap();
        assert!(json.contains("keychain:"));
    }
}
