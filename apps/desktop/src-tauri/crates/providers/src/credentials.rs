//! Credential storage in a local JSON config file. Secrets never touch logs.
//! Env vars remain as a fallback for headless/CI shells.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("no credential stored for {0}")]
    Missing(String),
}

fn store_path() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            std::path::PathBuf::from(home).join(".config")
        });
    base.join("aix").join("credentials.json")
}

fn read_store() -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(store_path())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&s).ok())
        .unwrap_or_default()
}

fn write_store(map: &serde_json::Map<String, serde_json::Value>) -> Result<(), CredentialError> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CredentialError::Storage(e.to_string()))?;
    }
    let body = serde_json::to_string_pretty(map).map_err(|e| CredentialError::Storage(e.to_string()))?;
    std::fs::write(&path, body).map_err(|e| CredentialError::Storage(e.to_string()))?;
    // owner-only — the file holds plaintext provider keys
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Store a secret. Returns a reference string persisted in app state.
pub fn store(provider_id: &str, secret: &str) -> Result<String, CredentialError> {
    let mut map = read_store();
    map.insert(provider_id.to_string(), serde_json::Value::String(secret.to_string()));
    write_store(&map)?;
    Ok(format!("config:credentials.json/{provider_id}"))
}

/// Env var checked as a fallback when the config file has no entry. This exists
/// for headless environments and CI — the file remains the primary store.
/// Never log the return value.
fn env_fallback(provider_id: &str) -> Option<String> {
    // primary: provider-derived name; aliases cover ecosystem conventions
    let mut vars = vec![format!("{}_API_KEY", provider_id.to_uppercase())];
    if provider_id == "qwen" {
        vars.push("DASHSCOPE_API_KEY".into());
    }
    vars.iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.trim().is_empty())
}

/// Retrieve a secret for use in an Authorization header. Never log the return value.
pub fn retrieve(provider_id: &str) -> Result<String, CredentialError> {
    let file_result = read_store()
        .get(provider_id)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| CredentialError::Missing(provider_id.to_string()));
    match file_result {
        Ok(secret) => Ok(secret),
        Err(e) => env_fallback(provider_id).ok_or(e),
    }
}

/// True when a key is retrievable from either the config file or env fallback.
pub fn available(provider_id: &str) -> bool {
    retrieve(provider_id).is_ok()
}

pub fn delete(provider_id: &str) -> Result<(), CredentialError> {
    let mut map = read_store();
    map.remove(provider_id);
    write_store(&map)
}

#[cfg(test)]
mod tests {
    #[test]
    fn env_fallback_reads_uppercased_provider_var() {
        std::env::set_var("TESTPROVIDER_API_KEY", "sk-test-123");
        assert_eq!(super::env_fallback("testprovider"), Some("sk-test-123".into()));
        std::env::remove_var("TESTPROVIDER_API_KEY");
        assert_eq!(super::env_fallback("testprovider"), None);
    }

    #[test]
    fn env_fallback_ignores_empty() {
        std::env::set_var("EMPTYPROV_API_KEY", "   ");
        assert_eq!(super::env_fallback("emptyprov"), None);
        std::env::remove_var("EMPTYPROV_API_KEY");
    }

    #[test]
    fn json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("aix-cred-test-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        super::store("roundtripprov", "sk-roundtrip").unwrap();
        assert_eq!(super::retrieve("roundtripprov").unwrap(), "sk-roundtrip");
        // overwrite
        super::store("roundtripprov", "sk-v2").unwrap();
        assert_eq!(super::retrieve("roundtripprov").unwrap(), "sk-v2");
        assert!(super::available("roundtripprov"));
        // persists across reads (fresh parse each time — no in-process caching)
        assert_eq!(super::retrieve("roundtripprov").unwrap(), "sk-v2");
        super::delete("roundtripprov").unwrap();
        assert!(super::retrieve("roundtripprov").is_err());
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
