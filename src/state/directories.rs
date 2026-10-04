//! `/api/directories` cache, the list behind the download-folder picker.
//!
//! Folder creation, moves and settings changes make it stale. Those live in different
//! entities, so staleness is a process-wide generation counter: [`invalidate`] bumps it and a
//! cached list only counts while it was fetched under the current generation.

use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{Context, Task};

use crate::api::ApiClient;
use crate::runtime;

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Call after anything that can change the directory list.
pub fn invalidate() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

#[derive(Default)]
pub struct DirectoriesStore {
    pub client: Option<ApiClient>,
    cached: Option<(u64, Vec<String>)>,
}

impl DirectoriesStore {
    /// The last list, unless something invalidated it since.
    pub fn cached(&self) -> Option<Vec<String>> {
        self.cached
            .as_ref()
            .filter(|(g, _)| *g == generation())
            .map(|(_, d)| d.clone())
    }

    /// Fetch the list. The cache is only filled when nothing invalidated it while the request was in
    /// flight; the caller still gets the answer either way.
    pub fn refresh(&mut self, cx: &mut Context<Self>) -> Task<Result<Vec<String>, String>> {
        let Some(client) = self.client.clone() else {
            return Task::ready(Err("The server is not running.".into()));
        };
        let started = generation();
        cx.spawn(async move |this, cx| {
            let dirs = runtime::run(async move { client.directories().await })
                .await
                .map_err(|e| e.to_string())?;
            this.update(cx, |s, _| {
                if started == generation() {
                    s.cached = Some((started, dirs.clone()));
                }
            })
            .ok();
            Ok(dirs)
        })
    }
}

/// Case-insensitive, slash-normalised form used to compare paths.
fn normalize_path(dir: &str) -> String {
    dir.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Drop every directory that lies under another one in the list ("Sub-folders" off).
pub fn top_level_dirs(dirs: &[String]) -> Vec<String> {
    let norm: Vec<String> = dirs.iter().map(|d| normalize_path(d)).collect();
    dirs.iter()
        .enumerate()
        .filter(|(i, _)| {
            !norm
                .iter()
                .enumerate()
                .any(|(j, other)| j != *i && norm[*i].starts_with(&format!("{other}/")))
        })
        .map(|(_, d)| d.clone())
        .collect()
}

/// Directories passing the "Sub-folders" toggle and the search box.
pub fn filter_dirs(all: &[String], query: &str, subfolders: bool) -> Vec<String> {
    let candidates = if subfolders {
        all.to_vec()
    } else {
        top_level_dirs(all)
    };
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return candidates;
    }
    candidates
        .into_iter()
        .filter(|d| d.to_lowercase().contains(&q))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> Vec<String> {
        [
            "C:\\Comics",
            "C:\\Comics\\Batman",
            "C:\\Comics\\Batman\\Year One",
            "D:\\Downloads",
            "C:\\ComicsExtra",
        ]
        .map(String::from)
        .to_vec()
    }

    #[test]
    fn top_level_drops_nested_paths_only() {
        // `C:\ComicsExtra` merely shares a prefix with `C:\Comics`: it is not nested.
        assert_eq!(
            top_level_dirs(&dirs()),
            ["C:\\Comics", "D:\\Downloads", "C:\\ComicsExtra"]
        );
    }

    #[test]
    fn top_level_ignores_case_and_trailing_slashes() {
        let d = vec!["/data/Comics/".to_string(), "/DATA/comics/x".to_string()];
        assert_eq!(top_level_dirs(&d), ["/data/Comics/"]);
    }

    #[test]
    fn filter_applies_toggle_then_search() {
        assert_eq!(filter_dirs(&dirs(), "", true).len(), 5);
        assert_eq!(filter_dirs(&dirs(), "batman", true).len(), 2);
        assert_eq!(filter_dirs(&dirs(), "batman", false).len(), 0);
        assert_eq!(filter_dirs(&dirs(), "  DOWN ", false), ["D:\\Downloads"]);
    }

    #[test]
    fn invalidating_discards_the_cache() {
        let mut s = DirectoriesStore::default();
        s.cached = Some((generation(), vec!["a".into()]));
        assert_eq!(s.cached(), Some(vec!["a".to_string()]));
        invalidate();
        assert_eq!(s.cached(), None);
    }
}
