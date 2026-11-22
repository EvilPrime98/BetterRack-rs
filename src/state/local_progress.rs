//! Last page read per local comic archive. Client-only, persisted in
//! `<config dir>/BetterRack/local_progress.json` as a map from canonical path to 1-based page.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn store_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("BetterRack")
            .join("local_progress.json"),
    )
}

fn key(file: &Path) -> Option<String> {
    Some(
        std::fs::canonicalize(file)
            .ok()?
            .to_string_lossy()
            .into_owned(),
    )
}

fn load_from(store: &Path) -> HashMap<String, u32> {
    std::fs::read_to_string(store)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_to(store: &Path, map: &HashMap<String, u32>) {
    if let Some(dir) = store.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(map) {
        if let Err(e) = std::fs::write(store, json) {
            tracing::warn!("could not save local progress to {}: {e}", store.display());
        }
    }
}

/// Page to reopen `file` on, for an archive of `total` pages: the saved page clamped to the
/// archive, or 1 when none is saved or the saved page was the last one.
pub fn resume_page(file: &Path, total: u32) -> u32 {
    let (Some(store), Some(k)) = (store_path(), key(file)) else {
        return 1;
    };
    resume_from(&load_from(&store), &k, total)
}

fn resume_from(map: &HashMap<String, u32>, k: &str, total: u32) -> u32 {
    match map.get(k) {
        Some(&p) if total > 0 => {
            let p = p.clamp(1, total);
            if p == total { 1 } else { p }
        }
        _ => 1,
    }
}

/// Best effort write of the page reached in `file`.
pub fn save_page(file: &Path, page: u32) {
    let (Some(store), Some(k)) = (store_path(), key(file)) else {
        return;
    };
    let mut map = load_from(&store);
    map.insert(k, page);
    save_to(&store, &map);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_missing_and_corrupt() {
        let dir = std::env::temp_dir().join(format!("br_local_progress_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("p.json");
        assert!(load_from(&file).is_empty());

        let mut map = HashMap::new();
        map.insert("a".to_string(), 7);
        save_to(&file, &map);
        assert_eq!(load_from(&file), map);

        std::fs::write(&file, "not json").unwrap();
        assert!(load_from(&file).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resume_rules() {
        let mut map = HashMap::new();
        map.insert("mid".to_string(), 5);
        map.insert("last".to_string(), 10);
        map.insert("past".to_string(), 99);
        assert_eq!(resume_from(&map, "mid", 10), 5);
        assert_eq!(resume_from(&map, "last", 10), 1);
        assert_eq!(resume_from(&map, "past", 10), 1);
        assert_eq!(resume_from(&map, "none", 10), 1);
    }
}
