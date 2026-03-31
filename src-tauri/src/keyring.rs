use anyhow::{Context, Result};

const SERVICE_NAME: &str = "com.revisor.app";
const TOKEN_KEY: &str = "github_token";

/// Store a token in the OS keyring
pub fn store_token(token: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, TOKEN_KEY)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(token)
        .context("Failed to store token in keyring")?;
    Ok(())
}

/// Retrieve the token from the OS keyring
pub fn get_token() -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, TOKEN_KEY)
        .context("Failed to create keyring entry")?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to read token from keyring: {}", e)),
    }
}

/// Delete the token from the OS keyring
pub fn delete_token() -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, TOKEN_KEY)
        .context("Failed to create keyring entry")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already gone
        Err(e) => Err(anyhow::anyhow!("Failed to delete token from keyring: {}", e)),
    }
}
