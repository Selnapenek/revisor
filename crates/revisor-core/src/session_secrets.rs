//! Optional API keys kept only until Revisor exits.

use anyhow::Result;
use std::sync::Mutex;

static FOCUS_KEY: Mutex<Option<String>> = Mutex::new(None);

fn normalize_focus_key(key: &str) -> Result<&str> {
    let key = key.trim();
    if key.is_empty() {
        anyhow::bail!("TypeSafe API key is required");
    }
    Ok(key)
}

pub fn store_focus_key(key: &str) -> Result<()> {
    *FOCUS_KEY.lock().expect("TypeSafe key lock poisoned") =
        Some(normalize_focus_key(key)?.to_owned());
    Ok(())
}

pub fn get_focus_key() -> Result<Option<String>> {
    Ok(FOCUS_KEY
        .lock()
        .expect("TypeSafe key lock poisoned")
        .clone())
}

pub fn delete_focus_key() -> Result<()> {
    *FOCUS_KEY.lock().expect("TypeSafe key lock poisoned") = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{delete_focus_key, get_focus_key, store_focus_key};

    #[test]
    fn focus_key_is_available_only_until_removed() {
        delete_focus_key().unwrap();
        assert!(store_focus_key(" \n ").is_err());
        store_focus_key("  test-key  ").unwrap();
        assert_eq!(get_focus_key().unwrap().as_deref(), Some("test-key"));
        delete_focus_key().unwrap();
        assert_eq!(get_focus_key().unwrap(), None);
    }
}
