//! Credential storage in the OS keychain. Secrets never touch disk or logs.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("no credential stored for {0}")]
    Missing(String),
}

const SERVICE_PREFIX: &str = "aix/providers";

/// Store a secret in the OS keychain. Returns the reference to persist in app state.
pub fn store(provider_id: &str, secret: &str) -> Result<String, CredentialError> {
    let key_ref = format!("keychain:{SERVICE_PREFIX}/{provider_id}");
    let entry = keyring::Entry::new(SERVICE_PREFIX, provider_id)
        .map_err(|e| CredentialError::Keychain(e.to_string()))?;
    entry
        .set_password(secret)
        .map_err(|e| CredentialError::Keychain(e.to_string()))?;
    Ok(key_ref)
}

/// Env var checked as a fallback when the keychain has no entry. This exists for
/// headless Linux (no secret-service), CI, and dev shells — the keychain remains
/// the primary store. Never log the return value.
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
    let entry = keyring::Entry::new(SERVICE_PREFIX, provider_id)
        .map_err(|e| CredentialError::Keychain(e.to_string()));
    let keychain_result = entry.and_then(|e| {
        e.get_password().map_err(|err| match err {
            keyring::Error::NoEntry => CredentialError::Missing(provider_id.to_string()),
            other => CredentialError::Keychain(other.to_string()),
        })
    });
    match keychain_result {
        Ok(secret) => Ok(secret),
        Err(e) => env_fallback(provider_id).ok_or(e),
    }
}

/// True when a key is retrievable from either keychain or env fallback.
pub fn available(provider_id: &str) -> bool {
    retrieve(provider_id).is_ok()
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
}

pub fn delete(provider_id: &str) -> Result<(), CredentialError> {
    let entry = keyring::Entry::new(SERVICE_PREFIX, provider_id)
        .map_err(|e| CredentialError::Keychain(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| CredentialError::Keychain(e.to_string()))
}
