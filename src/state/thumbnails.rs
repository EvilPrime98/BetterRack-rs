//! Cover images. GPUI's own HTTP client is a null client, so thumbnails are fetched through
//! [`ApiClient`] (which also handles the API key) and handed to `img()` as decoded-on-demand
//! `Arc<Image>`. Concurrency is bounded and the cache is an LRU so thousands of covers do not pile
//! up in memory.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::OnceLock;

use gpui::{Context, Image, ImageFormat};
use tokio::sync::Semaphore;

use crate::api::ApiClient;
use crate::runtime;

const MAX_CACHED: usize = 400;
const MAX_CONCURRENT: usize = 6;

fn permits() -> &'static Arc<Semaphore> {
    static S: OnceLock<Arc<Semaphore>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT)))
}

#[derive(Clone)]
pub enum Thumb {
    Loading,
    Ready(Arc<Image>),
    /// Server had no cover (or it could not be decoded): show the placeholder.
    Missing,
}

#[derive(Default)]
pub struct Thumbnails {
    pub client: Option<ApiClient>,
    entries: HashMap<String, Thumb>,
    /// Oldest first; only `Ready` entries are evicted.
    order: VecDeque<String>,
}

/// Keys are either server uids or absolute image URLs.
fn is_external(key: &str) -> bool {
    key.starts_with("http://") || key.starts_with("https://")
}

pub(crate) fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [0xff, 0xd8, 0xff, ..] => Some(ImageFormat::Jpeg),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some(ImageFormat::Webp),
        [b'G', b'I', b'F', ..] => Some(ImageFormat::Gif),
        [b'B', b'M', ..] => Some(ImageFormat::Bmp),
        _ => None,
    }
}

impl Thumbnails {
    pub fn get(&self, uid: &str) -> Option<&Thumb> {
        self.entries.get(uid)
    }

    /// Start loading `uid` if nobody has. Idempotent: safe to call from a list's render.
    pub fn ensure(&mut self, uid: &str, cx: &mut Context<Self>) {
        if self.entries.contains_key(uid) {
            return;
        }
        self.fetch(uid.to_string(), cx);
    }

    /// Like [`ensure`](Self::ensure) for an image hosted outside the server (store covers). The URL
    /// is the key; it cannot collide with a uid.
    pub fn ensure_external(&mut self, url: &str, cx: &mut Context<Self>) {
        if self.entries.contains_key(url) {
            return;
        }
        self.fetch(url.to_string(), cx);
    }

    /// Drop and re-fetch (after the server re-generated the thumbnail).
    pub fn reload(&mut self, uid: &str, cx: &mut Context<Self>) {
        self.entries.remove(uid);
        self.order.retain(|u| u != uid);
        self.fetch(uid.to_string(), cx);
    }

    fn fetch(&mut self, uid: String, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        self.entries.insert(uid.clone(), Thumb::Loading);
        cx.spawn(async move |this, cx| {
            let id = uid.clone();
            let bytes = runtime::run(async move {
                let _permit = permits().acquire().await.ok()?;
                if is_external(&id) {
                    client.external_bytes(&id).await.ok()
                } else {
                    client.thumbnail_bytes(&id).await.ok()
                }
            })
            .await;
            let thumb = match bytes.and_then(|b| sniff(&b).map(|f| (f, b))) {
                Some((format, bytes)) => Thumb::Ready(Arc::new(Image::from_bytes(format, bytes))),
                None => {
                    tracing::debug!("no usable image for {uid}");
                    Thumb::Missing
                }
            };
            this.update(cx, |s, cx| {
                if matches!(thumb, Thumb::Ready(_)) {
                    s.order.push_back(uid.clone());
                    while s.order.len() > MAX_CACHED {
                        if let Some(old) = s.order.pop_front() {
                            s.entries.remove(&old);
                        }
                    }
                }
                s.entries.insert(uid, thumb);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_external_and_uids_are_not() {
        assert!(is_external("https://cdn.example/a.jpg"));
        assert!(!is_external("3f9a1c"));
    }

    #[test]
    fn sniffs_common_formats() {
        assert!(matches!(
            sniff(&[0x89, b'P', b'N', b'G', 0]),
            Some(ImageFormat::Png)
        ));
        assert!(matches!(
            sniff(&[0xff, 0xd8, 0xff, 0xe0]),
            Some(ImageFormat::Jpeg)
        ));
        assert!(matches!(
            sniff(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(ImageFormat::Webp)
        ));
        assert!(sniff(b"<html>").is_none());
    }
}
