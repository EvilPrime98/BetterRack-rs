//! Per-card identification (`useComicIdentification`): when a card scrolls into view and the
//! library entry says `identified == None`, ask the server (`GET /api/library/:uid/identify`) and
//! remember the answer. Also carries the `lastIdentified`/`lastUnidentified` broadcast so the
//! originating card updates after the identify modal commits.

use std::collections::HashMap;

use gpui::Context;

use crate::api::ApiClient;
use crate::model::{LibraryEntry, MetaSource, WikiComic};
use crate::runtime;

#[derive(Clone, Debug)]
pub enum Ident {
    Loading,
    Resolved {
        comic: Option<WikiComic>,
        meta_source: Option<MetaSource>,
        identified: bool,
    },
}

#[derive(Default)]
pub struct IdentifyStore {
    pub client: Option<ApiClient>,
    states: HashMap<String, Ident>,
}

/// What a card should show for an entry: the lazily resolved state wins over the scan-time one.
pub struct CardInfo {
    pub comic: Option<WikiComic>,
    pub meta_source: Option<MetaSource>,
    pub identified: bool,
    pub loading: bool,
}

impl IdentifyStore {
    pub fn info(&self, item: &LibraryEntry) -> CardInfo {
        match self.states.get(&item.uid) {
            Some(Ident::Loading) => CardInfo {
                comic: item.comic.clone(),
                meta_source: item.meta_source,
                identified: item.identified != Some(false),
                loading: true,
            },
            Some(Ident::Resolved {
                comic,
                meta_source,
                identified,
            }) => CardInfo {
                comic: comic.clone(),
                meta_source: *meta_source,
                identified: *identified,
                loading: false,
            },
            None => CardInfo {
                comic: item.comic.clone(),
                meta_source: item.meta_source,
                identified: item.identified != Some(false),
                loading: item.identified.is_none(),
            },
        }
    }

    /// Called from the virtualized list for rows that are on screen, so it is idempotent.
    pub fn ensure(&mut self, item: &LibraryEntry, cx: &mut Context<Self>) {
        if item.did || item.identified.is_some() || self.states.contains_key(&item.uid) {
            return;
        }
        let Some(client) = self.client.clone() else {
            return;
        };
        let uid = item.uid.clone();
        self.states.insert(uid.clone(), Ident::Loading);
        cx.spawn(async move |this, cx| {
            let id = uid.clone();
            let resolved = runtime::run(async move { client.identify_lazy(&id).await }).await;
            this.update(cx, |s, cx| {
                // A failed lookup is not retried until the library is reloaded but the card stops spinning.
                let state = match resolved {
                    Ok(r) => Ident::Resolved {
                        comic: r.comic,
                        meta_source: r.meta_source,
                        identified: r.identified == Some(true),
                    },
                    Err(_) => Ident::Resolved {
                        comic: None,
                        meta_source: None,
                        identified: false,
                    },
                };
                s.states.insert(uid, state);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// What the card currently shows for `uid`, so a failed commit can put it back.
    pub fn snapshot(&self, uid: &str) -> Option<Ident> {
        self.states.get(uid).cloned()
    }

    pub fn restore(&mut self, uid: &str, previous: Option<Ident>, cx: &mut Context<Self>) {
        match previous {
            Some(state) => self.states.insert(uid.to_string(), state),
            None => self.states.remove(uid),
        };
        cx.notify();
    }

    /// Identify modal committed a pick, or the user un-identified.
    pub fn set_identified(
        &mut self,
        uid: &str,
        comic: Option<WikiComic>,
        meta: Option<MetaSource>,
        cx: &mut Context<Self>,
    ) {
        let identified = comic.is_some();
        self.states.insert(
            uid.to_string(),
            Ident::Resolved {
                comic,
                meta_source: meta,
                identified,
            },
        );
        cx.notify();
    }

    /// Forget everything (library refreshed/identified in bulk) so cards re-resolve.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.states.clear();
        cx.notify();
    }
}
