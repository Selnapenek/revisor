use anyhow::{Context, Result};

const SERVICE_NAME: &str = "com.revisor.app";
const TOKEN_KEY: &str = "github_token";
const FOCUS_KEY: &str = "typesafe_api_key";

fn normalize_focus_key(key: &str) -> Result<&str> {
    let key = key.trim();
    if key.is_empty() {
        anyhow::bail!("TypeSafe API key is required");
    }
    Ok(key)
}

pub fn store_focus_key(key: &str) -> Result<()> {
    let key = normalize_focus_key(key)?;
    keyring::Entry::new(SERVICE_NAME, FOCUS_KEY)
        .context("Failed to create TypeSafe key entry")?
        .set_password(key)
        .context("Failed to store TypeSafe key")
}

pub fn get_focus_key() -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, FOCUS_KEY)
        .context("Failed to create TypeSafe key entry")?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => anyhow::bail!("Failed to read TypeSafe key"),
    }
}

pub fn delete_focus_key() -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, FOCUS_KEY)
        .context("Failed to create TypeSafe key entry")?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => anyhow::bail!("Failed to delete TypeSafe key"),
    }
}

/// Store a token in the OS keyring
pub fn store_token(token: &str) -> Result<()> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, TOKEN_KEY).context("Failed to create keyring entry")?;
    entry
        .set_password(token)
        .context("Failed to store token in keyring")?;
    Ok(())
}

/// Retrieve the token from the OS keyring
pub fn get_token() -> Result<Option<String>> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, TOKEN_KEY).context("Failed to create keyring entry")?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to read token from keyring: {}", e)),
    }
}

/// Delete the token from the OS keyring
pub fn delete_token() -> Result<()> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, TOKEN_KEY).context("Failed to create keyring entry")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already gone
        Err(e) => Err(anyhow::anyhow!(
            "Failed to delete token from keyring: {}",
            e
        )),
    }
}

#[cfg(test)]
mod focus_tests {
    use super::normalize_focus_key;

    #[test]
    fn rejects_blank_focus_key_and_trims_valid_key() {
        assert!(normalize_focus_key(" \n ").is_err());
        assert_eq!(normalize_focus_key("  test-key  ").unwrap(), "test-key");
    }
}
