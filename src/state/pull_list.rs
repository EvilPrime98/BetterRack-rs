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

/// The title's series name equals the keyword (case-insensitive). "Batman" matches "Batman #4"
/// but not "Batman Beyond 012".
pub fn matches(entry: &PullEntry, title: &str) -> bool {
    let keyword = normalize(entry.keyword.trim());
    !keyword.is_empty() && normalize(&keyword_from_title(title)) == keyword
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
    pub fn add(&mut self, keyword: &str, cx: &mut Context<Self>) -> bool {
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
            last_seen_id: 0,
            created_at,
        });
        self.commit(cx);
        true
    }

    /// Whether any followed entry matches the post title.
    pub fn follows(&self, title: &str) -> bool {
        self.items.iter().any(|e| matches(e, title))
    }

    /// Unfollows every entry matching the title when there is one, else follows `keyword`.
    /// Returns whether the title is followed now.
    pub fn toggle(&mut self, title: &str, keyword: &str, cx: &mut Context<Self>) -> bool {
        if self.follows(title) {
            Rc::make_mut(&mut self.items).retain(|e| !matches(e, title));
            self.commit(cx);
            false
        } else {
            self.add(keyword, cx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(keyword: &str, seen: i64) -> PullEntry {
        PullEntry {
            keyword: keyword.into(),
            last_seen_id: seen,
            ..Default::default()
        }
    }

    #[test]
    fn keyword_is_the_case_insensitive_series_name() {
        let e = entry("batman", 0);
        assert!(matches(&e, "BATMAN #3"));
        assert!(matches(&e, "Batman 012 (2024)"));
        assert!(!matches(&e, "Batman_Beyond 012 (2024)"));
        assert!(!matches(&e, "The Dark Knight Batman #3"));
        assert!(!matches(&e, "Superman #3"));
        assert!(!matches(&entry("  ", 0), "anything"));
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
        let e = entry("Saga", 4);
        save_file(&file, std::slice::from_ref(&e));
        assert_eq!(load_file(&file), vec![e]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_file(Path::new("/not/here.json")).is_empty());
    }
}
