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

/// Retrieve a secret for use in an Authorization header. Never log the return value.
pub fn retrieve(provider_id: &str) -> Result<String, CredentialError> {
    let entry = keyring::Entry::new(SERVICE_PREFIX, provider_id)
        .map_err(|e| CredentialError::Keychain(e.to_string()))?;
    entry
        .get_password()
        .map_err(|e| match e {
            keyring::Error::NoEntry => CredentialError::Missing(provider_id.to_string()),
            other => CredentialError::Keychain(other.to_string()),
        })
}

pub fn delete(provider_id: &str) -> Result<(), CredentialError> {
    let entry = keyring::Entry::new(SERVICE_PREFIX, provider_id)
        .map_err(|e| CredentialError::Keychain(e.to_string()))?;
    entry
        .delete_credential()
        .map_err(|e| CredentialError::Keychain(e.to_string()))
}
