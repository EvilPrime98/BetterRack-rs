//! DC/Marvel Fandom scraper: a Rust port of the parts of the `better-wiki` npm package that
//! BetterRack uses (`WikiModel` in the Bun server).
//!
//! * [`client`] is the generic MediaWiki client (`wiki()` in better-wiki).
//! * [`plugins`] are the `dc-fandom` / `marvel-fandom` comic builders.
//! * [`WikiService`] is `WikiModel`: it asks every configured wiki at once.
//! * [`fuse`] and [`jsort`] reproduce the title ranking (Fuse.js + V8's sort) so the same
//!   candidate wins as in the Bun server.

pub mod client;
pub mod fuse;
pub mod jsort;
pub mod plugins;
pub mod template;

use client::{ClientOptions, WikiClient};
use plugins::{Plugin, PluginClient};
use serde_json::Value;

pub use client::{Page, PageFlags};

/// The wikis the server searches, in priority order (`WIKI_URLS` in `types.ts`).
pub const WIKI_URLS: [&str; 5] = [
    "https://dc.fandom.com",
    "https://marvel.fandom.com",
    "https://imagecomics.fandom.com",
    "https://darkhorse.fandom.com",
    "https://dynamiteentertainment.fandom.com",
];

const MARVEL_WIKI_URL: &str = "https://marvel.fandom.com";
const DEFAULT_THUMBNAIL_SIZE: &str = "120";

#[derive(Debug, thiserror::Error)]
pub enum WikiError {
    #[error("{0}")]
    Http(String),
}

pub type Result<T> = std::result::Result<T, WikiError>;

/// `thumbnailSize ? Number(thumbnailSize) : undefined`, as the JS-formatted width the client takes.
pub fn thumbnail_size_param(raw: Option<&str>) -> Option<String> {
    raw.filter(|r| !r.is_empty()).map(|r| template::js_number_string(template::js_number(r)))
}

pub fn is_known_wiki(url: &str) -> bool {
    WIKI_URLS.contains(&url)
}

/// `WikiModel`: one plugin client per wiki; the DC plugin serves every wiki but Marvel's.
pub struct WikiService {
    clients: Vec<PluginClient>,
}

impl Default for WikiService {
    fn default() -> Self {
        let wikis: Vec<(String, Plugin)> = WIKI_URLS.iter().map(|u| (u.to_string(), if *u == MARVEL_WIKI_URL { Plugin::Marvel } else { Plugin::Dc })).collect();
        Self::with_wikis(&wikis, reqwest::Client::new(), ClientOptions::default())
    }
}

impl WikiService {
    /// Explicit wiki list and network options; tests point this at a local mock.
    pub fn with_wikis(wikis: &[(String, Plugin)], http: reqwest::Client, opts: ClientOptions) -> Self {
        Self { clients: wikis.iter().map(|(url, plugin)| PluginClient { client: WikiClient::new(url, http.clone(), opts.clone()), plugin: *plugin }).collect() }
    }

    /// Best match for a title across all wikis: the first wiki (in order) that found one.
    /// Any wiki failing fails the lookup, like `Promise.all`.
    pub async fn get_comic(&self, title: &str, thumbnail_size: Option<&str>) -> Result<Option<Value>> {
        let size = thumbnail_size.unwrap_or(DEFAULT_THUMBNAIL_SIZE);
        tracing::info!("Searching wiki info for: {title}");
        let results = futures_util::future::try_join_all(self.clients.iter().map(|c| c.find_comic(title, Some(size)))).await?;
        Ok(results.into_iter().flatten().next())
    }

    /// Every hit across all wikis, wiki by wiki.
    pub async fn get_comics(&self, title: &str, thumbnail_size: Option<&str>) -> Result<Vec<Value>> {
        let size = thumbnail_size.unwrap_or(DEFAULT_THUMBNAIL_SIZE);
        let results = futures_util::future::try_join_all(self.clients.iter().map(|c| c.find_comics(title, Some(size)))).await?;
        Ok(results.into_iter().flatten().collect())
    }

    /// A comic by page id on one wiki; `None` when the wiki is not configured or the page is missing.
    pub async fn get_comic_by_id(&self, page_id: i64, source_wiki: &str, thumbnail_size: Option<&str>) -> Result<Option<Value>> {
        let size = thumbnail_size.unwrap_or(DEFAULT_THUMBNAIL_SIZE);
        match self.clients.iter().find(|c| c.client.wiki_url() == source_wiki) {
            Some(c) => c.find_comic_by_id(page_id, Some(size)).await,
            None => Ok(None),
        }
    }
}
