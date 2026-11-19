//! Pull-list: followed title keywords, owned by the server (`/api/pull-list`). This store is a view
//! of the server list; matching runs on the posts the Store has already loaded.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::Context;

use crate::api::ApiClient;
use crate::model::PullEntry;
use crate::runtime;
use crate::ui::toast;

/// The desktop's old local list, imported into the server once and then renamed.
fn legacy_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("BetterRack")
            .join("pull-list.json"),
    )
}

fn load_legacy(path: &Path) -> Vec<PullEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Pushes the old local entries to the server (which dedupes by keyword), then renames the file
/// so it is not imported again. On any failure the file stays for the next start.
async fn import_legacy(client: &ApiClient) {
    let Some(path) = legacy_path().filter(|p| p.exists()) else {
        return;
    };
    for e in load_legacy(&path) {
        if let Err(err) = client.add_pull_entry(&e.keyword, e.last_seen_id).await {
            tracing::warn!("pull-list import failed, keeping {}: {err}", path.display());
            return;
        }
    }
    if let Err(e) = std::fs::rename(&path, path.with_extension("json.migrated")) {
        tracing::warn!("could not rename {}: {e}", path.display());
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

#[derive(Default)]
pub struct PullListStore {
    pub client: Option<ApiClient>,
    pub items: Rc<Vec<PullEntry>>,
}

impl PullListStore {
    /// Imports the legacy local file, then loads the server list. Failures are logged: the
    /// list stays empty until the next start.
    pub fn init(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = runtime::run(async move {
                import_legacy(&client).await;
                client.pull_list().await
            })
            .await;
            this.update(cx, |s, cx| match result {
                Ok(items) => {
                    s.items = Rc::new(items);
                    cx.notify();
                }
                Err(e) => tracing::warn!("loading the pull list failed: {e}"),
            })
            .ok();
        })
        .detach();
    }

    /// Follows `keyword` on the server; an already followed one comes back as the existing entry.
    /// The list changes only on success, otherwise an error toast is shown.
    pub fn add(&mut self, keyword: &str, cx: &mut Context<Self>) {
        let keyword = keyword.trim().to_string();
        if keyword.is_empty() {
            return;
        }
        let Some(client) = self.client.clone() else {
            toast::error(cx, "The server is not running.");
            return;
        };
        cx.spawn(async move |this, cx| {
            let kw = keyword.clone();
            let result = runtime::run(async move { client.add_pull_entry(&kw, 0).await }).await;
            this.update(cx, |s, cx| match result {
                Ok(entry) => {
                    let items = Rc::make_mut(&mut s.items);
                    match items.iter_mut().find(|e| e.id == entry.id) {
                        Some(existing) => *existing = entry,
                        None => items.push(entry),
                    }
                    cx.notify();
                    toast::success(cx, format!("Added “{keyword}” to your pull list."));
                }
                Err(e) => toast::error(cx, format!("Could not update the pull list: {e}")),
            })
            .ok();
        })
        .detach();
    }

    /// Whether any followed entry matches the post title.
    pub fn follows(&self, title: &str) -> bool {
        self.items.iter().any(|e| matches(e, title))
    }

    /// Unfollows every entry matching the title when there is one, else follows `keyword`.
    pub fn toggle(&mut self, title: &str, keyword: &str, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .items
            .iter()
            .filter(|e| matches(e, title))
            .map(|e| e.id.clone())
            .collect();
        if ids.is_empty() {
            self.add(keyword, cx);
            return;
        }
        let Some(client) = self.client.clone() else {
            toast::error(cx, "The server is not running.");
            return;
        };
        let keyword = keyword.to_string();
        cx.spawn(async move |this, cx| {
            let mut removed = Vec::new();
            let mut failure = None;
            for id in ids {
                let (c, i) = (client.clone(), id.clone());
                match runtime::run(async move { c.delete_pull_entry(&i).await }).await {
                    Ok(()) => removed.push(id),
                    Err(e) => {
                        failure = Some(e.to_string());
                        break;
                    }
                }
            }
            this.update(cx, |s, cx| {
                Rc::make_mut(&mut s.items).retain(|e| !removed.contains(&e.id));
                cx.notify();
                match failure {
                    None => toast::success(cx, format!("Removed “{keyword}” from your pull list.")),
                    Some(e) => toast::error(cx, format!("Could not update the pull list: {e}")),
                }
            })
            .ok();
        })
        .detach();
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
    fn legacy_file_is_read_without_exclude() {
        let dir = std::env::temp_dir().join(format!("br-pull-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pull-list.json");
        std::fs::write(&file, r#"[{"keyword":"Saga","lastSeenId":4}]"#).unwrap();
        assert_eq!(load_legacy(&file), vec![entry("Saga", 4)]);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(load_legacy(Path::new("/not/here.json")).is_empty());
    }
}
