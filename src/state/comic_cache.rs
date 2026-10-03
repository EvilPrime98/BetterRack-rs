//! `comicCache.store.ts`: per-comic progress/rating/read flag, loaded once at startup.
//! Writes update locally at once and debounce the PATCH 400 ms per uid. `flush_pending` must run
//! before views that read server-side progress (Reading) and on exit (gotcha #3).

use std::collections::HashMap;
use std::time::Duration;

use gpui::{AppContext as _, Context, Task};

use crate::api::ApiClient;
use crate::model::ComicCache;
use crate::runtime;

const DEBOUNCE: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct ComicCacheStore {
    pub client: Option<ApiClient>,
    pub cache: HashMap<String, ComicCache>,
    pub ready: bool,
    /// uid → the timer task that will send it. Replacing a task drops (cancels) the old timer.
    pending: HashMap<String, Task<()>>,
}

impl ComicCacheStore {
    /// Initial load (`comicCache.init`). Failures leave an empty, ready cache.
    pub fn init(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let loaded = match client {
                Some(c) => runtime::run(async move { c.comic_data().await }).await.ok(),
                None => None,
            };
            this.update(cx, |s, cx| {
                if let Some(map) = loaded {
                    s.cache = map;
                }
                s.ready = true;
                cx.notify();
            })
            .ok();
        })
    }

    pub fn get(&self, uid: &str) -> Option<&ComicCache> {
        self.cache.get(uid)
    }

    /// 0..=100; `read == true` forces 100 (as the React card does).
    pub fn read_per(&self, uid: &str) -> f32 {
        match self.cache.get(uid) {
            Some(c) if c.read => 100.0,
            Some(c) => c.read_per,
            None => 0.0,
        }
    }

    /// `setCacheById`: merge locally, then debounce the PATCH (the full merged object is sent).
    pub fn update(
        &mut self,
        uid: &str,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut ComicCache),
    ) {
        let entry = self.cache.entry(uid.to_string()).or_default();
        change(entry);
        cx.notify();

        let uid = uid.to_string();
        let key = uid.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let send = this
                .update(cx, |s, _| {
                    s.pending.remove(&uid);
                    s.client.clone().zip(s.cache.get(&uid).cloned())
                })
                .ok()
                .flatten();
            if let Some((client, data)) = send {
                let id = uid.clone();
                if let Err(e) = runtime::run(async move { client.patch_comic_data(&id, &data).await }).await {
                    tracing::warn!("saving progress for {uid} failed: {e}");
                }
            }
        });
        self.pending.insert(key, task);
    }

    pub fn set_rating(&mut self, uid: &str, rating: f32, cx: &mut Context<Self>) {
        self.update(uid, cx, |c| c.rating = rating);
    }

    /// Mark read/unread (`mark-as-read-button`): read pins progress to 100, unread resets it.
    pub fn set_read(&mut self, uid: &str, read: bool, cx: &mut Context<Self>) {
        self.update(uid, cx, |c| {
            c.read = read;
            c.read_per = if read { 100.0 } else { 0.0 };
            if !read {
                c.current_page = 0;
            }
        });
    }

    /// Send everything still waiting on a debounce timer. The returned task completes when the
    /// last PATCH has been answered.
    pub fn flush_pending(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let client = self.client.clone();
        let batch: Vec<(String, ComicCache)> = self
            .pending
            .drain()
            .filter_map(|(uid, _timer)| self.cache.get(&uid).cloned().map(|c| (uid, c)))
            .collect();
        cx.background_spawn(async move {
            let Some(client) = client else { return };
            for (uid, data) in batch {
                let c = client.clone();
                let id = uid.clone();
                if let Err(e) = runtime::run(async move { c.patch_comic_data(&id, &data).await }).await {
                    tracing::warn!("flushing progress for {uid} failed: {e}");
                }
            }
        })
    }
}
