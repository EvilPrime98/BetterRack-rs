//! Wiki lookup seam. `Library::identify` falls back to the wiki when a file has no usable
//! `ComicInfo.xml` and the `wikiSearch` setting is on, and `/api/wiki/*` serves the same
//! lookups. The methods are blocking (call them from `spawn_blocking` or a plain thread);
//! the real implementation, `br-wiki`, is async and is bridged by `br-server`.
//!
//! `thumbnail_size` is the JS-formatted width (`"120"`) or `None` for the default of 120.

use crate::Result;
use serde_json::Value;

pub trait WikiLookup: Send + Sync {
    /// Best match for a file name across all wikis, as a `WikiComic` JSON object, or `None`.
    fn get_comic(&self, title: &str, thumbnail_size: Option<&str>) -> Result<Option<Value>>;

    /// Every match across all wikis.
    fn get_comics(&self, _title: &str, _thumbnail_size: Option<&str>) -> Result<Vec<Value>> {
        Ok(vec![])
    }

    /// One comic by page id on `source_wiki`; `None` when the page or the wiki is unknown.
    fn get_comic_by_id(
        &self,
        _page_id: i64,
        _source_wiki: &str,
        _thumbnail_size: Option<&str>,
    ) -> Result<Option<Value>> {
        Ok(None)
    }
}

/// Lookups off: nothing is ever found.
pub struct NoWiki;

impl WikiLookup for NoWiki {
    fn get_comic(&self, _title: &str, _thumbnail_size: Option<&str>) -> Result<Option<Value>> {
        Ok(None)
    }
}
