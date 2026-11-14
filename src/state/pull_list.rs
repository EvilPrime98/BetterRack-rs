//! Pull-list: followed title keywords persisted to `pull-list.json` in the config dir (separate
//! from the server's own file). Matching runs on the posts the Store has already loaded.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::Context;

use crate::model::PullEntry;

fn path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("BetterRack")
            .join("pull-list.json"),
    )
}

fn load_file(path: &Path) -> Vec<PullEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_file(path: &Path, items: &[PullEntry]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(items) {
        if let Err(e) = std::fs::write(path, json) {
            tracing::warn!("could not save pull-list to {}: {e}", path.display());
        }
    }
}

fn normalize(title: &str) -> String {
    title.replace('_', " ").to_lowercase()
}

/// Case-insensitive substring of the title, unless the title contains any exclude term.
pub fn matches(entry: &PullEntry, title: &str) -> bool {
    let keyword = entry.keyword.trim().to_lowercase();
    if keyword.is_empty() {
        return false;
    }
    let title = normalize(title);
    title.contains(&keyword)
        && !entry
            .exclude
            .split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .any(|t| title.contains(&t))
}

/// A keyword guess from a post title: the title up to the issue number or parenthesis.
pub fn keyword_from_title(title: &str) -> String {
    let t = title.replace('_', " ");
    let end = t.find(['#', '(']).unwrap_or(t.len());
    t[..end]
        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace() || c == '-')
        .trim()
        .to_string()
}

pub struct PullListStore {
    pub items: Rc<Vec<PullEntry>>,
    file: Option<PathBuf>,
}

impl PullListStore {
    pub fn load() -> Self {
        let file = path();
        let items = file.as_deref().map(load_file).unwrap_or_default();
        Self {
            items: Rc::new(items),
            file,
        }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if let Some(file) = &self.file {
            save_file(file, &self.items);
        }
        cx.notify();
    }

    /// An already followed keyword (case-insensitive) is left alone.
    pub fn add(&mut self, keyword: &str, exclude: &str, cx: &mut Context<Self>) -> bool {
        let keyword = keyword.trim();
        if keyword.is_empty()
            || self
                .items
                .iter()
                .any(|e| e.keyword.to_lowercase() == keyword.to_lowercase())
        {
            return false;
        }
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let id = format!("{created_at:x}-{}", self.items.len());
        Rc::make_mut(&mut self.items).push(PullEntry {
            id,
            keyword: keyword.to_string(),
            exclude: exclude.trim().to_string(),
            last_seen_id: 0,
            created_at,
        });
        self.commit(cx);
        true
    }

    pub fn remove(&mut self, id: &str, cx: &mut Context<Self>) {
        Rc::make_mut(&mut self.items).retain(|e| e.id != id);
        self.commit(cx);
    }

    pub fn follows(&self, keyword: &str) -> bool {
        self.find(keyword).is_some()
    }

    fn find(&self, keyword: &str) -> Option<&PullEntry> {
        let keyword = keyword.trim().to_lowercase();
        self.items
            .iter()
            .find(|e| e.keyword.to_lowercase() == keyword)
    }

    /// Follows the keyword, or unfollows it when already followed. Returns whether it is followed now.
    pub fn toggle(&mut self, keyword: &str, cx: &mut Context<Self>) -> bool {
        if let Some(id) = self.find(keyword).map(|e| e.id.clone()) {
            self.remove(&id, cx);
            false
        } else {
            self.add(keyword, "", cx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(keyword: &str, exclude: &str, seen: i64) -> PullEntry {
        PullEntry {
            keyword: keyword.into(),
            exclude: exclude.into(),
            last_seen_id: seen,
            ..Default::default()
        }
    }

    #[test]
    fn keyword_is_a_case_insensitive_substring() {
        let e = entry("batman", "", 0);
        assert!(matches(&e, "The_Dark Knight BATMAN #3"));
        assert!(!matches(&e, "Superman #3"));
        assert!(!matches(&entry("  ", "", 0), "anything"));
    }

    #[test]
    fn any_exclude_term_rejects() {
        let e = entry("Batman", "annual, Special ,", 0);
        assert!(matches(&e, "Batman #4"));
        assert!(!matches(&e, "Batman Annual #1"));
        assert!(!matches(&e, "Batman: SPECIAL"));
    }

    #[test]
    fn keyword_guess_drops_the_issue_part() {
        assert_eq!(
            keyword_from_title("Batman_Beyond 012 (2024)"),
            "Batman Beyond"
        );
        assert_eq!(keyword_from_title("Saga #54"), "Saga");
        assert_eq!(keyword_from_title("Maus"), "Maus");
    }

    #[test]
    fn entries_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("br-pull-{}", std::process::id()));
        let file = dir.join("pull-list.json");
        let e = entry("Saga", "x", 4);
        save_file(&file, std::slice::from_ref(&e));
        assert_eq!(load_file(&file), vec![e]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_file(Path::new("/not/here.json")).is_empty());
    }
}
