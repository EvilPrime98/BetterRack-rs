//! Store page state (`stores/store.store.ts` + the data flow of `store.page.tsx` and
//! `store-card.tsx`). It lives at app level so the query, results, page and scroll position survive
//! leaving and re-entering the page.
//!
//! Per-card download state also lives here: GPUI rebuilds cards as they scroll in and out of the
//! virtualized list, so a card cannot own it.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Context, UniformListScrollHandle};

use crate::api::ApiClient;
use crate::model::{STORE_PAGE_SIZE, StartDownload, StorePost, StoreStrat};
use crate::runtime;
use crate::ui::{modals, toast};

/// `TCardState`. While links are loading the button keeps its previous look in React; here it
/// ignores clicks.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CardState {
    #[default]
    Idle,
    LinksLoading,
    Error(String),
}

pub struct StoreState {
    pub client: Option<ApiClient>,
    /// The text box.
    pub query: String,
    /// The term the current results belong to.
    pub active_search: String,
    pub results: Rc<Vec<StorePost>>,
    pub page: u32,
    pub has_more: bool,
    pub has_loaded: bool,
    pub loading: bool,
    pub loading_more: bool,
    /// Loading the next page failed; stops the scroll from retrying in a loop until the user does.
    pub more_failed: bool,
    pub error: String,
    pub scroll: UniformListScrollHandle,
    cards: HashMap<String, CardState>,
    /// Bumped by every new search so late answers for an older one are dropped.
    request: u64,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            client: None,
            query: String::new(),
            active_search: String::new(),
            results: Rc::new(Vec::new()),
            page: 1,
            has_more: true,
            has_loaded: false,
            loading: false,
            loading_more: false,
            more_failed: false,
            error: String::new(),
            scroll: UniformListScrollHandle::new(),
            cards: HashMap::new(),
            request: 0,
        }
    }
}

/// Stable identity of a post (`keyFor`).
pub fn post_key(post: &StorePost) -> String {
    match post.id {
        Some(id) => format!("id:{id}"),
        None => format!("link:{}", post.link),
    }
}

/// Append `incoming` to `existing`, skipping posts already present (the source can repeat an item
/// across page boundaries when new posts arrive in between).
pub fn append_unique(existing: &[StorePost], incoming: Vec<StorePost>) -> Vec<StorePost> {
    let mut seen: std::collections::HashSet<String> = existing.iter().map(post_key).collect();
    let mut out = existing.to_vec();
    out.extend(incoming.into_iter().filter(|p| seen.insert(post_key(p))));
    out
}

/// The upload date as shown under a card: the `YYYY-MM-DD` part of an ISO timestamp, otherwise the
/// text as sent (React used the browser's locale date).
pub fn display_date(raw: &str) -> String {
    let day = raw.get(..10).filter(|d| {
        let b = d.as_bytes();
        b[4] == b'-' && b[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    });
    day.unwrap_or(raw).to_string()
}

impl StoreState {
    pub fn card(&self, key: &str) -> CardState {
        self.cards.get(key).cloned().unwrap_or_default()
    }

    fn set_card(&mut self, key: &str, state: CardState, cx: &mut Context<Self>) {
        if state == CardState::Idle {
            self.cards.remove(key);
        } else {
            self.cards.insert(key.to_string(), state);
        }
        cx.notify();
    }

    /// First visit: show the "latest" feed. A no-op once something has loaded.
    pub fn ensure_loaded(&mut self, cx: &mut Context<Self>) {
        if !self.has_loaded && !self.loading {
            self.load_first(self.active_search.clone(), cx);
        }
    }

    /// The Search button / Enter.
    pub fn run_search(&mut self, cx: &mut Context<Self>) {
        let term = self.query.trim().to_string();
        self.active_search = term.clone();
        self.scroll = UniformListScrollHandle::new();
        self.load_first(term, cx);
    }

    fn fetch_page(client: ApiClient, term: String, page: u32) -> impl std::future::Future<Output = Result<Vec<StorePost>, String>> {
        runtime::run(async move {
            let search = (!term.is_empty()).then_some(term.as_str());
            client.store_posts(search, page, STORE_PAGE_SIZE).await.map_err(|e| e.to_string())
        })
    }

    fn load_first(&mut self, term: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else { return };
        self.request += 1;
        let request = self.request;
        self.loading = true;
        self.loading_more = false;
        self.more_failed = false;
        self.error.clear();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = Self::fetch_page(client, term, 1).await;
            this.update(cx, |s, cx| {
                if s.request != request {
                    return;
                }
                s.loading = false;
                match result {
                    Ok(items) => {
                        s.has_more = items.len() == STORE_PAGE_SIZE;
                        s.results = Rc::new(items);
                        s.page = 1;
                        s.has_loaded = true;
                    }
                    Err(message) => {
                        s.error = if message.is_empty() { "Failed to load comics.".into() } else { message };
                        toast::error(cx, s.error.clone());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Idempotent: the list calls it whenever its last rows are on screen.
    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        if !self.has_more || !self.has_loaded || self.loading || self.loading_more || self.more_failed {
            return;
        }
        let Some(client) = self.client.clone() else { return };
        let (term, next, request) = (self.active_search.clone(), self.page + 1, self.request);
        self.loading_more = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = Self::fetch_page(client, term, next).await;
            this.update(cx, |s, cx| {
                // A new search started meanwhile: these rows belong to the old one.
                if s.request != request {
                    return;
                }
                s.loading_more = false;
                match result {
                    Ok(items) => {
                        s.has_more = items.len() == STORE_PAGE_SIZE;
                        s.results = Rc::new(append_unique(&s.results, items));
                        s.page = next;
                    }
                    Err(message) => {
                        s.more_failed = true;
                        toast::error(cx, if message.is_empty() { "Failed to load more comics.".to_string() } else { message });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn retry_more(&mut self, cx: &mut Context<Self>) {
        self.more_failed = false;
        self.load_more(cx);
    }

    /// The card's Download / Retry button: links → (link picker) → folder picker → job.
    pub fn download(&mut self, post: &StorePost, cx: &mut Context<Self>) {
        let Some(id) = post.id.filter(|id| *id != 0) else { return };
        let Some(client) = self.client.clone() else { return };
        let key = post_key(post);
        if self.card(&key) == CardState::LinksLoading {
            return;
        }
        self.set_card(&key, CardState::LinksLoading, cx);
        let title = post.title.clone();

        cx.spawn(async move |this, cx| {
            let outcome = Self::download_flow(&client, id, &title, cx).await;
            this.update(cx, |s, cx| match outcome {
                Flow::Started => {
                    s.set_card(&key, CardState::Idle, cx);
                    toast::success(cx, "Download in progress");
                }
                Flow::Cancelled => s.set_card(&key, CardState::Idle, cx),
                // Like React, "no links" is shown on the card only.
                Flow::NoLinks => s.set_card(&key, CardState::Error("No download links found.".into()), cx),
                Flow::Failed(message) => {
                    toast::error(cx, message.clone());
                    s.set_card(&key, CardState::Error(message), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    async fn download_flow(client: &ApiClient, id: i64, title: &str, cx: &mut gpui::AsyncApp) -> Flow {
        let links = {
            let client = client.clone();
            runtime::run(async move { client.comic_links(id).await }).await
        };
        let links = match links {
            Ok(l) => l,
            Err(e) => return Flow::Failed(fallback(e.to_string(), "Failed to fetch links.")),
        };
        let link = match links.len() {
            0 => return Flow::NoLinks,
            1 => links.into_iter().next().expect("one link"),
            _ => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let title = title.to_string();
                cx.update(|cx| {
                    modals::open_link_picker(cx, links, title, move |picked, _| {
                        let _ = tx.send(picked);
                    })
                });
                match rx.await {
                    Ok(Some(link)) => link,
                    _ => return Flow::Cancelled,
                }
            }
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        cx.update(|cx| {
            modals::open_download_dir(cx, move |dir, _| {
                let _ = tx.send(dir);
            })
        });
        let Ok(Some(output_dir)) = rx.await else { return Flow::Cancelled };

        let req = StartDownload { id, title: link.title, uuid: link.uuid, output_dir, strat: StoreStrat::All };
        let client = client.clone();
        match runtime::run(async move { client.start_download(&req).await }).await {
            Ok(_) => Flow::Started,
            Err(e) => Flow::Failed(fallback(e.to_string(), "Download failed.")),
        }
    }
}

enum Flow {
    Started,
    Cancelled,
    NoLinks,
    Failed(String),
}

fn fallback(message: String, default: &str) -> String {
    if message.is_empty() { default.to_string() } else { message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(id: Option<i64>, link: &str) -> StorePost {
        StorePost { id, thumbnail_url: None, title: link.into(), link: link.into(), upload_date: None }
    }

    #[test]
    fn keys_prefer_the_id() {
        assert_eq!(post_key(&post(Some(7), "x")), "id:7");
        assert_eq!(post_key(&post(None, "https://s/x")), "link:https://s/x");
    }

    #[test]
    fn appending_skips_repeats() {
        let a = vec![post(Some(1), "a"), post(Some(2), "b")];
        let out = append_unique(&a, vec![post(Some(2), "b"), post(Some(3), "c"), post(Some(3), "c")]);
        let ids: Vec<_> = out.iter().map(|p| p.id).collect();
        assert_eq!(ids, [Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn dates_show_the_day() {
        assert_eq!(display_date("2025-03-09T12:30:00"), "2025-03-09");
        assert_eq!(display_date("March 9, 2025"), "March 9, 2025");
        assert_eq!(display_date("2025"), "2025");
        assert_eq!(display_date(""), "");
    }
}
