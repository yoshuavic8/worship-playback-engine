//! Secrets live in the macOS Keychain, never in the settings file.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;

const SERVICE: &str = "app.worshipplayback.engine";
const FREELLM_ACCOUNT: &str = "freellmapi-token";

/// Session cache so the Keychain is read at most once per launch.
static CACHE: Mutex<Option<Option<String>>> = Mutex::new(None);

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, FREELLM_ACCOUNT).map_err(|e| anyhow!("Keychain unavailable: {e}"))
}

pub fn freellm_token() -> Option<String> {
    let mut c = CACHE.lock();
    if let Some(v) = c.as_ref() {
        return v.clone();
    }
    let v = entry().ok().and_then(|e| e.get_password().ok()).filter(|t| !t.trim().is_empty());
    *c = Some(v.clone());
    v
}

pub fn set_freellm_token(token: &str) -> Result<()> {
    let t = token.trim();
    if t.is_empty() {
        return clear_freellm_token();
    }
    entry()?.set_password(t).map_err(|e| anyhow!("Could not save the token in Keychain: {e}"))?;
    *CACHE.lock() = Some(Some(t.to_string()));
    Ok(())
}

pub fn clear_freellm_token() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(anyhow!("Could not remove the token from Keychain: {e}")),
    }
    *CACHE.lock() = Some(None);
    Ok(())
}
