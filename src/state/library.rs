//! Library state: groups, structure, search, the 5-minute stale cache with in-flight
//! de-dup, every mutation invalidating it, and the identify-all poller.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{AsyncApp, Context, Task, WeakEntity};

use crate::api::ApiClient;
use crate::model::{
    FilterOption, IDENTIFY_POLL_INTERVAL_MS, IdentifyLibraryStatus, IdentifyProgress, JobState,
    LibraryEntry, LibraryGroup, LibraryStructure,
};
use crate::runtime;
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::toast;

const STALE_TIME: Duration = Duration::from_secs(60 * 5);

#[derive(Default)]
pub struct LibraryStore {
    pub client: Option<ApiClient>,
    pub groups: Vec<LibraryGroup>,
    pub structure: LibraryStructure,
    pub search_query: String,
    /// `Some((done, total))` while the identify-all job runs (`total == 0` until known).
    pub identify_progress: Option<(u32, u32)>,
    /// Broadcast for views that keep their own snapshot (Recent/Reading): each delete bumps the
    /// counter so repeated deletes of the same uid stay distinct.
    pub last_deleted: Option<(u64, String)>,
    /// A rescan started from the sidebar button is running.
    pub refreshing: bool,
    pub loading: bool,
    /// Bumped whenever `groups`, `structure` or `search_query` change, so views can memoize.
    pub revision: u64,
    cache: HashMap<LibraryStructure, (Vec<LibraryGroup>, Instant)>,
    /// structure → generation the in-flight request belongs to.
    in_flight: HashMap<LibraryStructure, u64>,
    generation: u64,
    delete_count: u64,
    identify_polling: bool,
}

impl LibraryStore {
    fn invalidate(&mut self) {
        // Folders were created/moved/removed: the download-folder list is stale too.
        crate::state::directories::invalidate();
        self.cache.clear();
        // An older in-flight response must not repopulate the cache we just emptied.
        self.generation += 1;
    }

    pub fn set_search_query(&mut self, q: impl Into<String>, cx: &mut Context<Self>) {
        self.search_query = q.into();
        self.revision += 1;
        cx.notify();
    }

    pub fn set_structure(&mut self, structure: LibraryStructure, cx: &mut Context<Self>) {
        if self.structure == structure {
            return;
        }
        self.structure = structure;
        self.revision += 1;
        if let Some((groups, _)) = self.cache.get(&structure) {
            self.groups = groups.clone();
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            if let Err(e) = Self::load(&this, cx).await {
                cx.update(|cx| toast::error(cx, e));
            }
        })
        .detach();
    }

    /// `fetchLibrary()`: fresh cache wins; otherwise one request per structure at a time.
    pub fn fetch(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            if let Err(e) = Self::load(&this, cx).await {
                cx.update(|cx| toast::error(cx, e));
            }
        })
    }

    async fn load(this: &WeakEntity<Self>, cx: &mut AsyncApp) -> Result<(), String> {
        enum Plan {
            Done,
            Fetch(ApiClient, LibraryStructure, u64),
        }
        let plan = this
            .update(cx, |s, cx| {
                let structure = s.structure;
                if let Some((groups, at)) = s.cache.get(&structure) {
                    if at.elapsed() < STALE_TIME && !s.in_flight.contains_key(&structure) {
                        let groups = groups.clone();
                        if s.groups != groups {
                            s.groups = groups;
                            s.revision += 1;
                            cx.notify();
                        }
                        return Plan::Done;
                    }
                }
                if s.in_flight.get(&structure) == Some(&s.generation) {
                    return Plan::Done;
                }
                let Some(client) = s.client.clone() else {
                    return Plan::Done;
                };
                s.in_flight.insert(structure, s.generation);
                s.loading = true;
                cx.notify();
                Plan::Fetch(client, structure, s.generation)
            })
            .map_err(|e| e.to_string())?;
        let Plan::Fetch(client, structure, generation) = plan else {
            return Ok(());
        };

        let by_series = structure == LibraryStructure::Series;
        let result = runtime::run(async move { client.library_all(by_series).await }).await;
        this.update(cx, |s, cx| {
            if s.in_flight.get(&structure) == Some(&generation) {
                s.in_flight.remove(&structure);
            }
            s.loading = !s.in_flight.is_empty();
            if let Ok(groups) = &result {
                if generation == s.generation {
                    s.cache.insert(structure, (groups.clone(), Instant::now()));
                    if s.structure == structure {
                        s.groups = groups.clone();
                        s.revision += 1;
                    }
                }
            }
            cx.notify();
        })
        .map_err(|e| e.to_string())?;
        result.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Run a server mutation, then invalidate + refetch and toast. `fallback` is used when the
    /// server sent no message of its own.
    fn mutate<F, Fut>(
        &mut self,
        failure: &'static str,
        success: &'static str,
        cx: &mut Context<Self>,
        call: F,
    ) where
        F: FnOnce(ApiClient) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = crate::api::ApiResult<()>> + Send + 'static,
    {
        self.mutate_then(failure, success, cx, call, |_, _| {});
    }

    fn mutate_then<F, Fut>(
        &mut self,
        failure: &'static str,
        success: &'static str,
        cx: &mut Context<Self>,
        call: F,
        after: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) where
        F: FnOnce(ApiClient) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = crate::api::ApiResult<()>> + Send + 'static,
    {
        let Some(client) = self.client.clone() else {
            return;
        };
        cx.spawn(
            async move |this, cx| match runtime::run(call(client)).await {
                Ok(()) => {
                    this.update(cx, |s, cx| {
                        s.invalidate();
                        after(s, cx);
                    })
                    .ok();
                    let refetched = Self::load(&this, cx).await;
                    cx.update(|cx| match refetched {
                        Ok(()) => toast::success(cx, success),
                        Err(e) => toast::error(cx, e),
                    });
                }
                Err(e) => {
                    let msg = if e.to_string().is_empty() {
                        failure.to_string()
                    } else {
                        e.to_string()
                    };
                    cx.update(|cx| toast::error(cx, msg));
                }
            },
        )
        .detach();
    }

    pub fn delete_file(&mut self, uid: String, cx: &mut Context<Self>) {
        let broadcast = uid.clone();
        self.mutate_then(
            "Failed to delete file.",
            "File deleted",
            cx,
            move |c| async move { c.delete_file(&uid).await },
            move |s, _| {
                s.delete_count += 1;
                s.last_deleted = Some((s.delete_count, broadcast));
            },
        );
    }

    pub fn delete_folder(&mut self, uid: String, cx: &mut Context<Self>) {
        let broadcast = uid.clone();
        self.mutate_then(
            "Failed to delete folder.",
            "Folder deleted",
            cx,
            move |c| async move { c.delete_folder(&uid).await },
            move |s, _| {
                s.delete_count += 1;
                s.last_deleted = Some((s.delete_count, broadcast));
            },
        );
    }

    pub fn create_folder(&mut self, name: String, parent: Option<String>, cx: &mut Context<Self>) {
        self.mutate(
            "Failed to create folder.",
            "Folder created",
            cx,
            move |c| async move { c.create_folder(&name, parent.as_deref()).await },
        );
    }

    pub fn move_file(&mut self, uid: String, target: Option<String>, cx: &mut Context<Self>) {
        self.mutate(
            "Failed to move file.",
            "File moved",
            cx,
            move |c| async move { c.move_file(&uid, target.as_deref()).await },
        );
    }

    pub fn unidentify_file(&mut self, uid: String, cx: &mut Context<Self>) {
        let broadcast = uid.clone();
        self.mutate_then(
            "Failed to un-identify file.",
            "File un-identified",
            cx,
            move |c| async move { c.unidentify_file(&uid).await },
            // `lastUnidentified`: the originating card drops its metadata without a reload.
            move |_, cx| {
                if let Some(stores) = cx.try_global::<crate::state::Stores>().cloned() {
                    stores
                        .identify
                        .update(cx, |s, cx| s.set_identified(&broadcast, None, None, cx));
                }
            },
        );
    }

    pub fn reidentify_all(&mut self, cx: &mut Context<Self>) {
        self.mutate_then(
            "Failed to flag library for re-identification.",
            "Library flagged for re-identification",
            cx,
            |c| async move { c.identify_reset_all().await },
            |_, cx| clear_identify_states(cx),
        );
    }

    /// `refreshLibrary`: rescan, then refetch.
    pub fn refresh(&mut self, silent: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let ok = Self::rescan(&this, cx).await;
            if ok && !silent {
                cx.update(|cx| toast::success(cx, "Library refreshed"));
            }
        })
        .detach();
    }

    async fn rescan(this: &WeakEntity<Self>, cx: &mut AsyncApp) -> bool {
        let Ok(Some(client)) = this.update(cx, |s, cx| {
            s.refreshing = true;
            cx.notify();
            s.client.clone()
        }) else {
            return false;
        };
        let result = runtime::run(async move { client.library_refresh().await }).await;
        let outcome = match result {
            Ok(()) => {
                this.update(cx, |s, _| s.invalidate()).ok();
                Self::load(this, cx).await
            }
            Err(e) => Err(e.to_string()),
        };
        this.update(cx, |s, cx| {
            s.refreshing = false;
            cx.notify();
        })
        .ok();
        match outcome {
            Ok(()) => true,
            Err(e) => {
                cx.update(|cx| toast::error(cx, e));
                false
            }
        }
    }

    /// `refreshLibraryWithPrompt`: refresh, then offer to identify everything.
    pub fn refresh_with_prompt(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if !Self::rescan(&this, cx).await {
                return;
            }
            let empty = this
                .update(cx, |s, _| s.groups.iter().all(|g| g.entries.is_empty()))
                .unwrap_or(true);
            if empty {
                cx.update(|cx| toast::success(cx, "Library refreshed"));
                return;
            }
            let this = this.clone();
            cx.update(|cx| {
                confirm::ask(
                    cx,
                    ConfirmOptions::new(
                        "Identify library?",
                        "Do you also want to identify every comic in your library after refreshing? \
                         This can take a while and runs in the background.",
                    )
                    .labels("Refresh & identify", "Just refresh"),
                    move |answer, _, cx| match answer {
                        None => {}
                        Some(false) => toast::success(cx, "Library refreshed"),
                        Some(true) => {
                            this.update(cx, |s, cx| s.identify_library(cx).detach()).ok();
                        }
                    },
                );
            });
        })
        .detach();
    }

    /// Start the identify-all job and poll it every second until it ends.
    pub fn identify_library(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if self.identify_polling {
            return Task::ready(());
        }
        let Some(client) = self.client.clone() else {
            return Task::ready(());
        };
        self.identify_polling = true;
        cx.spawn(async move |this, cx| {
            let outcome = Self::poll_identify(&this, cx, client).await;
            this.update(cx, |s, cx| {
                s.identify_polling = false;
                s.identify_progress = None;
                cx.notify();
            })
            .ok();
            match outcome {
                Ok(true) => cx.update(|cx| toast::success(cx, "Library identified")),
                Ok(false) => {}
                Err(e) => cx.update(|cx| toast::error(cx, e)),
            }
        })
    }

    /// `Ok(true)` finished, `Ok(false)` nothing to do, `Err` failed.
    async fn poll_identify(
        this: &WeakEntity<Self>,
        cx: &mut AsyncApp,
        client: ApiClient,
    ) -> Result<bool, String> {
        let start = client.clone();
        let mut status = runtime::run(async move { start.identify_all_start().await })
            .await
            .map_err(|e| e.to_string())?;
        loop {
            let job = match status {
                IdentifyLibraryStatus::Idle => return Ok(false),
                IdentifyLibraryStatus::Job(job) => job,
            };
            if let Some(IdentifyProgress::Error { message }) = &job.progress {
                return Err(message.clone());
            }
            if job.state == JobState::Error {
                return Err("Library identification failed.".into());
            }
            if job.state == JobState::Done
                || matches!(job.progress, Some(IdentifyProgress::Done { .. }))
            {
                this.update(cx, |s, cx| {
                    s.invalidate();
                    clear_identify_states(cx);
                })
                .map_err(|e| e.to_string())?;
                Self::load(this, cx).await?;
                return Ok(true);
            }
            let progress = match job.progress {
                Some(IdentifyProgress::Identifying { done, total }) => (done, total),
                _ => (0, 0),
            };
            this.update(cx, |s, cx| {
                s.identify_progress = Some(progress);
                cx.notify();
            })
            .map_err(|e| e.to_string())?;
            cx.background_executor()
                .timer(Duration::from_millis(IDENTIFY_POLL_INTERVAL_MS))
                .await;
            let poll = client.clone();
            status = runtime::run(async move { poll.identify_all_status().await })
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    pub fn items(&self, only_dir: bool, uid: Option<&str>) -> Vec<LibraryEntry> {
        library_items(&self.groups, self.structure, only_dir, uid)
    }

    /// What the library page shows for `uid` (`None` = root): folders (+ loose root comics in
    /// folders mode), then the sidebar text filter.
    pub fn page_items(&self, uid: Option<&str>) -> Vec<LibraryEntry> {
        page_items(&self.groups, self.structure, uid, &self.search_query)
    }

    pub fn find_entry(&self, uid: &str) -> Option<&LibraryEntry> {
        self.groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .find(|e| e.uid == uid)
    }

    pub fn title_for(&self, uid: Option<&str>) -> String {
        let Some(uid) = uid else { return "Root".into() };
        self.find_entry(uid)
            .map(|e| e.name.clone())
            .or_else(|| {
                self.groups
                    .iter()
                    .find(|g| g.uid == uid)
                    .map(|g| g.name.clone())
            })
            .unwrap_or_else(|| "Root".into())
    }
}

/// Cards remember what the lazy identify (or a modal pick) told them; after a bulk identify or a
/// flag-for-re-identify those answers are stale, so cards re-resolve from the reloaded library.
fn clear_identify_states(cx: &mut gpui::App) {
    if let Some(stores) = cx.try_global::<crate::state::Stores>().cloned() {
        stores.identify.update(cx, |s, cx| s.clear(cx));
    }
}

/// `getLibraryItems({onlyDir, uid})`: the core of navigation.
pub fn library_items(
    groups: &[LibraryGroup],
    structure: LibraryStructure,
    only_dir: bool,
    uid: Option<&str>,
) -> Vec<LibraryEntry> {
    let series_dir = |g: &LibraryGroup| LibraryEntry {
        uid: g.uid.clone(),
        did: true,
        name: g.name.clone(),
        path: g.path.clone(),
        parent_id: String::new(),
        created_at: 0,
        identified: None,
        comic: None,
        meta_source: None,
    };

    if structure == LibraryStructure::Series {
        match uid {
            None => {
                return if only_dir {
                    groups.iter().map(series_dir).collect()
                } else {
                    groups
                        .iter()
                        .flat_map(|g| g.entries.iter().cloned())
                        .collect()
                };
            }
            Some(uid) => {
                if let Some(series) = groups.iter().find(|g| g.uid == uid) {
                    return if only_dir {
                        Vec::new()
                    } else {
                        series.entries.clone()
                    };
                }
                // Not a series uid: fall through to the folder lookup below.
            }
        }
    }

    let group = uid.and_then(|u| groups.iter().find(|g| g.uid == u));
    let mut data: Vec<LibraryEntry> = match (uid, group) {
        (None, _) => groups
            .iter()
            .flat_map(|g| g.entries.iter().cloned())
            .collect(),
        (Some(_), Some(g)) => g
            .entries
            .iter()
            .filter(|e| e.parent_id.is_empty())
            .cloned()
            .collect(),
        (Some(uid), None) => groups
            .iter()
            .flat_map(|g| g.entries.iter())
            .filter(|e| e.parent_id == uid)
            .cloned()
            .collect(),
    };
    if only_dir {
        data.retain(|e| e.did);
    }
    data
}

pub fn page_items(
    groups: &[LibraryGroup],
    structure: LibraryStructure,
    uid: Option<&str>,
    query: &str,
) -> Vec<LibraryEntry> {
    let mut items = library_items(groups, structure, uid.is_none(), uid);
    if uid.is_none() && structure == LibraryStructure::Folders {
        items.extend(
            groups
                .iter()
                .flat_map(|g| g.entries.iter())
                .filter(|e| !e.did && e.parent_id.is_empty())
                .cloned(),
        );
    }
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        items.retain(|i| i.name.to_lowercase().contains(&query));
    }
    items
}

/// Entries without a release date go last (keeping their relative
/// order), the rest ascend by year, month, day. `Creation Date` is newest first.
pub fn apply_sort(items: &mut [LibraryEntry], filter: FilterOption) {
    match filter {
        FilterOption::Alphabetically => {}
        FilterOption::CreationDate => items.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
        FilterOption::ReleaseDate => items.sort_by_key(|e| {
            let key = e
                .comic
                .as_ref()
                .and_then(|c| c.release_date.as_ref())
                .and_then(|d| d.sort_key());
            (key.is_none(), key)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(v: serde_json::Value) -> LibraryEntry {
        serde_json::from_value(v).unwrap()
    }

    fn groups() -> Vec<LibraryGroup> {
        vec![LibraryGroup {
            uid: "g".into(),
            name: "Root dir".into(),
            path: "/g".into(),
            entries: vec![
                entry(json!({"uid":"f1","did":true,"name":"Batman","parentId":""})),
                entry(json!({"uid":"f2","did":true,"name":"Nested","parentId":"f1"})),
                entry(json!({"uid":"c1","did":false,"name":"Loose","parentId":""})),
                entry(json!({"uid":"c2","did":false,"name":"Issue 1","parentId":"f1"})),
            ],
        }]
    }

    fn uids(v: &[LibraryEntry]) -> Vec<&str> {
        v.iter().map(|e| e.uid.as_str()).collect()
    }

    #[test]
    fn root_shows_folders_then_loose_comics() {
        let items = page_items(&groups(), LibraryStructure::Folders, None, "");
        assert_eq!(uids(&items), ["f1", "f2", "c1"]);
    }

    #[test]
    fn group_uid_lists_its_top_level_entries() {
        let items = library_items(&groups(), LibraryStructure::Folders, false, Some("g"));
        assert_eq!(uids(&items), ["f1", "c1"]);
    }

    #[test]
    fn folder_uid_lists_children() {
        let items = page_items(&groups(), LibraryStructure::Folders, Some("f1"), "");
        assert_eq!(uids(&items), ["f2", "c2"]);
    }

    #[test]
    fn search_is_case_insensitive_substring() {
        let items = page_items(&groups(), LibraryStructure::Folders, None, " BAT ");
        assert_eq!(uids(&items), ["f1"]);
    }

    #[test]
    fn series_root_lists_series_as_directories() {
        let dirs = library_items(&groups(), LibraryStructure::Series, true, None);
        assert_eq!(uids(&dirs), ["g"]);
        assert!(dirs[0].did);
        let all = library_items(&groups(), LibraryStructure::Series, false, None);
        assert_eq!(all.len(), 4);
        assert!(library_items(&groups(), LibraryStructure::Series, true, Some("g")).is_empty());
    }

    #[test]
    fn release_date_sort_puts_undated_last() {
        let mut items = vec![
            entry(json!({"uid":"none","name":"n"})),
            entry(
                json!({"uid":"late","name":"l","comic":{"releaseDate":{"releaseYear":"2001","releaseMonth":"2","releaseDay":"1"}}}),
            ),
            entry(
                json!({"uid":"early","name":"e","comic":{"releaseDate":{"releaseYear":"1999","releaseMonth":"12","releaseDay":"31"}}}),
            ),
        ];
        apply_sort(&mut items, FilterOption::ReleaseDate);
        assert_eq!(uids(&items), ["early", "late", "none"]);
    }
}
