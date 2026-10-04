//! Remote mode: the desktop app can talk to a
//! remote BetterRack deployment instead of spawning the local sidecar.
//!
//! The URL and the remote-mode flag live in `<config>/BetterRack/server.json` and the API key in the OS keychain
//! (`keyring`), never in a file.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "BetterRack";
const KEYRING_USER: &str = "server-api-key";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    /// Normalized remote URL (`http://host:port`), empty when never set.
    pub url: String,
    pub remote: bool,
}

impl ServerConfig {
    /// Remote mode is on but there is nothing to connect to (`needsServerSetup`).
    pub fn needs_setup(&self) -> bool {
        self.remote && self.url.is_empty()
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("BetterRack").join("server.json"))
}

pub fn load() -> ServerConfig {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The config as it was when the app started. Changing the server relaunches the app, so a process never sees it change.
pub fn current() -> &'static ServerConfig {
    static CURRENT: OnceLock<ServerConfig> = OnceLock::new();
    CURRENT.get_or_init(load)
}

/// Remote mode: the native folder picker is off, since paths refer to the remote machine
/// (`hasNativeFolderPicker() = picker && !remote`).
pub fn has_native_folder_picker() -> bool {
    !current().remote
}

fn save(config: &ServerConfig) -> Result<(), String> {
    let p = path().ok_or("no config directory on this system")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("could not write {}: {e}", p.display()))
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// The stored API key, if any. A keychain failure reads as "no key" (and is logged).
pub fn api_key() -> Option<String> {
    match entry().and_then(|e| e.get_password().map_err(|e| e.to_string())) {
        Ok(k) if !k.is_empty() => Some(k),
        Ok(_) => None,
        Err(e) => {
            // `NoEntry` is the normal "never set" case; anything else is worth a line in the log.
            if !e.to_lowercase().contains("no matching entry") {
                tracing::warn!("could not read the API key from the keychain: {e}");
            }
            None
        }
    }
}

fn store_api_key(key: &str) -> Result<(), String> {
    let entry = entry()?;
    if key.is_empty() {
        return match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    }
    entry
        .set_password(key)
        .map_err(|e| format!("could not store the API key in the keychain: {e}"))
}

/// `setRemoteServer(url, apiKey)`: pair the URL with the remote flag and the optional key.
pub fn set_remote(raw_url: &str, api_key: &str) -> Result<(), String> {
    store_api_key(api_key.trim())?;
    save(&ServerConfig {
        url: crate::api::normalize_base_url(raw_url),
        remote: true,
    })
}

/// `clearRemoteServer()`: back to the local sidecar.
pub fn clear_remote() -> Result<(), String> {
    save(&ServerConfig::default())?;
    store_api_key("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_setup_only_when_remote_without_url() {
        assert!(
            ServerConfig {
                url: String::new(),
                remote: true
            }
            .needs_setup()
        );
        assert!(
            !ServerConfig {
                url: "http://x".into(),
                remote: true
            }
            .needs_setup()
        );
        assert!(!ServerConfig::default().needs_setup());
    }

    #[test]
    fn partial_file_falls_back_to_local() {
        let c: ServerConfig = serde_json::from_str(r#"{"url":"http://nas:3000"}"#).unwrap();
        assert!(!c.remote);
        assert_eq!(c.url, "http://nas:3000");
    }
}
