use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::{App, Context};

use crate::model::{LibraryGroup, POLL_INTERVAL_MS, StoreBookmark, StorePost};
use crate::runtime;
use crate::state::Stores;
use crate::state::store::post_key;
use crate::ui::toast;

fn path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join("BetterRack")
            .join("bookmarks.json"),
    )
}

fn load_file(path: &Path) -> Vec<StoreBookmark> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_file(path: &Path, items: &[StoreBookmark]) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(items) {
        if let Err(e) = std::fs::write(path, json) {
            tracing::warn!("could not save bookmarks to {}: {e}", path.display());
        }
    }
}

pub struct BookmarksStore {
    items: Rc<Vec<StoreBookmark>>,
    downloading: HashSet<String>,
    file: Option<PathBuf>,
}

impl BookmarksStore {
    pub fn load() -> Self {
        let file = path();
        let items = file.as_deref().map(load_file).unwrap_or_default();
        Self {
            items: Rc::new(items),
            downloading: HashSet::new(),
            file,
        }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if let Some(file) = &self.file {
            save_file(file, &self.items);
        }
        cx.notify();
    }

    fn edit(&mut self, key: &str, cx: &mut Context<Self>, change: impl FnOnce(&mut StoreBookmark)) {
        let Some(b) = Rc::make_mut(&mut self.items)
            .iter_mut()
            .find(|b| b.key == key)
        else {
            return;
        };
        change(b);
        self.commit(cx);
    }

    pub fn get(&self, key: &str) -> Option<&StoreBookmark> {
        self.items.iter().find(|b| b.key == key)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn is_downloading(&self, key: &str) -> bool {
        self.downloading.contains(key)
    }

    pub fn in_folder(&self, folder: Option<&str>) -> Vec<StoreBookmark> {
        self.items
            .iter()
            .filter(|b| b.folder.as_deref() == folder)
            .cloned()
            .collect()
    }

    pub fn add(&mut self, post: &StorePost, folder: Option<String>, cx: &mut Context<Self>) {
        let key = post_key(post);
        if self.contains(&key) {
            self.move_to(&key, folder, cx);
            return;
        }
        Rc::make_mut(&mut self.items).push(StoreBookmark::new(key, post, folder));
        self.commit(cx);
    }

    pub fn remove(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.contains(key) {
            return;
        }
        Rc::make_mut(&mut self.items).retain(|b| b.key != key);
        self.commit(cx);
    }

    pub fn move_to(&mut self, key: &str, folder: Option<String>, cx: &mut Context<Self>) {
        self.edit(key, cx, |b| b.folder = folder);
    }

    pub fn set_rating(&mut self, key: &str, rating: f32, cx: &mut Context<Self>) {
        self.edit(key, cx, |b| b.rating = rating);
    }

    pub fn set_read(&mut self, key: &str, read: bool, cx: &mut Context<Self>) {
        self.edit(key, cx, |b| b.read = read);
    }
}

pub fn folder_dir(groups: &[LibraryGroup], folder: Option<&str>) -> Option<String> {
    let uid = folder?;
    groups
        .iter()
        .find(|g| g.uid == uid)
        .map(|g| g.path.clone())
        .or_else(|| {
            groups
                .iter()
                .flat_map(|g| g.entries.iter())
                .find(|e| e.did && e.uid == uid)
                .map(|e| e.path.clone())
        })
        .filter(|p| !p.is_empty())
}

fn comic_uids(groups: &[LibraryGroup]) -> HashSet<String> {
    groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| !e.did)
        .map(|e| e.uid.clone())
        .collect()
}

fn only_new_comic(groups: &[LibraryGroup], before: &HashSet<String>) -> Option<String> {
    let mut fresh = groups
        .iter()
        .flat_map(|g| g.entries.iter())
        .filter(|e| !e.did && !before.contains(&e.uid));
    let first = fresh.next()?;
    fresh.next().is_none().then(|| first.uid.clone())
}

pub fn download(stores: &Stores, key: &str, cx: &mut App) {
    let Some(bookmark) = stores.bookmarks.read(cx).get(key).cloned() else {
        return;
    };
    if stores.bookmarks.read(cx).is_downloading(key) {
        return;
    }
    if bookmark.id.is_none_or(|id| id == 0) {
        toast::error(cx, "This post cannot be downloaded.");
        return;
    }
    let groups = stores.library.read(cx).groups.clone();
    let dir = folder_dir(&groups, bookmark.folder.as_deref());
    let before = comic_uids(&groups);
    let Some(client) = stores.downloads.read(cx).client.clone() else {
        return;
    };

    stores.bookmarks.update(cx, |s, cx| {
        s.downloading.insert(bookmark.key.clone());
        cx.notify();
    });
    let started = {
        let stores = stores.clone();
        let key = bookmark.key.clone();
        move |job: Option<String>, cx: &mut App| match job {
            Some(job) => watch_job(stores, client, key, job, before, cx),
            None => finish(&stores, &key, cx),
        }
    };
    let post = bookmark.post();
    stores
        .store
        .update(cx, |s, cx| s.download_into(&post, dir, started, cx));
}

fn finish(stores: &Stores, key: &str, cx: &mut App) {
    stores.bookmarks.update(cx, |s, cx| {
        s.downloading.remove(key);
        cx.notify();
    });
}

fn watch_job(
    stores: Stores,
    client: crate::api::ApiClient,
    key: String,
    job: String,
    before: HashSet<String>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let done = loop {
            cx.background_executor()
                .timer(Duration::from_millis(POLL_INTERVAL_MS))
                .await;
            let c = client.clone();
            let Ok(jobs) = runtime::run(async move { c.download_jobs().await }).await else {
                continue;
            };
            match jobs.iter().find(|j| j.job_id == job).map(|j| j.state) {
                Some(crate::model::JobState::Done) => break true,
                Some(crate::model::JobState::Error) | None => break false,
                Some(_) => {}
            }
        };
        if !done {
            cx.update(|cx| finish(&stores, &key, cx));
            return;
        }
        let Some(bookmark) = cx.update(|cx| stores.bookmarks.read(cx).get(&key).cloned()) else {
            cx.update(|cx| finish(&stores, &key, cx));
            return;
        };
        let mut loaded = false;
        for _ in 0..5 {
            let task = cx.update(|cx| stores.library.update(cx, |l, cx| l.rescan_task(cx)));
            if task.await {
                loaded = true;
                break;
            }
            cx.background_executor().timer(Duration::from_secs(1)).await;
        }
        cx.update(|cx| {
            if loaded {
                let groups = stores.library.read(cx).groups.clone();
                if let Some(uid) = only_new_comic(&groups, &before) {
                    stores.comics.update(cx, |s, cx| {
                        if bookmark.rating > 0.0 {
                            s.set_rating(&uid, bookmark.rating, cx);
                        }
                        if bookmark.read {
                            s.set_read(&uid, true, cx);
                        }
                    });
                }
            }
            stores.bookmarks.update(cx, |s, cx| s.remove(&key, cx));
            finish(&stores, &key, cx);
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LibraryEntry;

    fn entry(uid: &str, did: bool, path: &str) -> LibraryEntry {
        serde_json::from_value(serde_json::json!({
            "uid": uid, "did": did, "name": uid, "path": path, "parentId": ""
        }))
        .unwrap()
    }

    fn groups() -> Vec<LibraryGroup> {
        vec![LibraryGroup {
            uid: "g".into(),
            name: "Root".into(),
            path: "/root".into(),
            entries: vec![
                entry("f", true, "/root/f"),
                entry("c", false, "/root/f/c.cbz"),
            ],
        }]
    }

    fn post(id: Option<i64>, link: &str) -> StorePost {
        StorePost {
            id,
            thumbnail_url: None,
            title: link.into(),
            link: link.into(),
            upload_date: None,
        }
    }

    #[test]
    fn folders_and_groups_resolve_to_their_directory() {
        let g = groups();
        assert_eq!(folder_dir(&g, Some("f")).as_deref(), Some("/root/f"));
        assert_eq!(folder_dir(&g, Some("g")).as_deref(), Some("/root"));
        assert_eq!(folder_dir(&g, Some("gone")), None);
        assert_eq!(folder_dir(&g, None), None);
        assert_eq!(folder_dir(&g, Some("c")), None);
    }

    #[test]
    fn a_single_new_comic_is_found() {
        let mut g = groups();
        let before = comic_uids(&g);
        assert_eq!(only_new_comic(&g, &before), None);
        g[0].entries.push(entry("n1", false, "/root/f/n1.cbz"));
        assert_eq!(only_new_comic(&g, &before).as_deref(), Some("n1"));
        g[0].entries.push(entry("n2", false, "/root/f/n2.cbz"));
        assert_eq!(only_new_comic(&g, &before), None);
    }

    #[test]
    fn bookmarks_round_trip_through_the_file() {
        let dir = std::env::temp_dir().join(format!("br-bookmarks-{}", std::process::id()));
        let file = dir.join("bookmarks.json");
        let mut b = StoreBookmark::new("id:7".into(), &post(Some(7), "x"), Some("f".into()));
        b.rating = 3.5;
        b.read = true;
        save_file(&file, std::slice::from_ref(&b));
        assert_eq!(load_file(&file), vec![b]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_broken_files_load_empty() {
        assert!(load_file(Path::new("/definitely/not/here.json")).is_empty());
    }
}
