//! The optional link to a BetterRack server. The URL lives in `<data>/server.json`, the API key in
//! the Keystore-sealed `server_key.bin` (see `vault`). Linked, the server is the source of truth;
//! unlinking returns to the on-device library, so the app stays usable offline.

use crate::backend::{Backend, normalize_base_url};
use crate::vault;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    url: String,
}

fn file(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("server.json")
}

/// The linked server URL, if any.
pub fn saved_url(data_dir: &Path) -> Option<String> {
    let saved: Saved = serde_json::from_slice(&std::fs::read(file(data_dir)).ok()?).ok()?;
    Some(saved.url).filter(|u| !u.is_empty())
}

/// Links the backend to the saved server, if there is one (no network call).
pub fn restore(data_dir: &Path, backend: &Backend) -> Option<String> {
    let url = saved_url(data_dir)?;
    backend.link_remote(&url, vault::load(data_dir)).ok()?;
    Some(url)
}

/// Checks the server answers with this key, then saves it and switches the backend over.
pub async fn connect(data_dir: &Path, backend: &Backend, raw_url: &str, api_key: &str) -> Result<String, String> {
    let url = normalize_base_url(raw_url);
    if raw_url.trim().is_empty() {
        return Err("enter the server address".into());
    }
    let key = api_key.trim();
    backend.link_remote(&url, Some(key.to_owned()))?;
    if let Err(e) = backend.get_json("/api/settings").await {
        backend.unlink_remote();
        return Err(if e.contains("401") { "the server rejected the API key".into() } else { e });
    }
    vault::save(data_dir, key).map_err(|e| format!("could not store the key: {e}"))?;
    let json = serde_json::to_vec(&Saved { url: url.clone() }).map_err(|e| e.to_string())?;
    std::fs::write(file(data_dir), json).map_err(|e| e.to_string())?;
    Ok(url)
}

pub fn disconnect(data_dir: &Path, backend: &Backend) {
    backend.unlink_remote();
    let _ = std::fs::remove_file(file(data_dir));
    let _ = vault::clear(data_dir);
}
