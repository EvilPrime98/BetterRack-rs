//! Reader page images One entity per open comic.
//!
//! Pages are fetched through [`ApiClient`] (bounded concurrency), their size is read from the
//! header only (so layout can reserve the right height before GPUI decodes anything), and ready
//! images far from the reading position are evicted to keep memory bounded. Aspect ratios outlive
//! the images so page heights stay stable after eviction.

use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use br_core::archive::Archives;
use gpui::{Context, Image, ImageSource, Task};
use tokio::sync::Semaphore;

use crate::api::ApiClient;
use crate::runtime;
use crate::state::thumbnails::sniff;

const MAX_CONCURRENT: usize = 4;
/// Ready images kept on each side of the reading position.
const KEEP_RADIUS: u32 = 10;

fn permits() -> &'static Arc<Semaphore> {
    static S: OnceLock<Arc<Semaphore>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Semaphore::new(MAX_CONCURRENT)))
}

#[derive(Clone)]
pub enum PageImg {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

/// Where the reader gets its pages from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderSource {
    /// A library comic, read through the server.
    Library { uid: String },
    /// An archive on disk outside the library, read in-process.
    LocalFile { path: PathBuf },
}

/// An opened local archive and its page entries, in reading order.
pub struct LocalArchive {
    path: PathBuf,
    archives: Archives,
    pages: Vec<String>,
}

impl LocalArchive {
    /// Blocking: lists the archive's image pages.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!("File not found: \"{}\"", path.display()));
        }
        let archives =
            Archives::new(std::env::var_os(br_core::archive::SEVEN_ZIP_ENV_VAR).map(PathBuf::from));
        let pages = archives.list_pages(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            archives,
            pages,
        })
    }

    pub fn len(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Blocking: the page bookmarks in `ComicInfo.xml`; empty on any failure.
    pub fn bookmarks(&self) -> Vec<crate::model::Bookmark> {
        self.archives
            .bookmarks(&self.path)
            .unwrap_or_default()
            .into_iter()
            .map(|b| crate::model::Bookmark {
                page: b.page,
                label: b.label,
            })
            .collect()
    }

    /// Bytes of the 1-based `page`.
    fn page_bytes(&self, page: u32) -> Option<Vec<u8>> {
        let entry = self.pages.get(page.checked_sub(1)? as usize)?;
        self.archives.read_entry(&self.path, entry).ok()
    }
}

enum Backend {
    Server(ApiClient, String),
    Local(Arc<LocalArchive>),
}

pub struct PageImages {
    source: ReaderSource,
    client: Option<ApiClient>,
    local: Option<Arc<LocalArchive>>,
    entries: HashMap<u32, PageImg>,
    /// height / width, by 1-based page.
    aspects: HashMap<u32, f32>,
    /// Pages whose aspect became known (or changed) since the view last looked.
    newly_sized: Vec<u32>,
    center: u32,
}

/// Image dimensions from the header; no full decode.
fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

impl PageImages {
    pub fn new(source: ReaderSource, client: Option<ApiClient>) -> Self {
        Self {
            source,
            client,
            local: None,
            entries: HashMap::new(),
            aspects: HashMap::new(),
            newly_sized: Vec::new(),
            center: 1,
        }
    }

    /// Local mode: the archive pages are read from.
    pub fn set_local(&mut self, local: Arc<LocalArchive>) {
        self.local = Some(local);
    }

    fn backend(&self) -> Option<Backend> {
        match &self.source {
            ReaderSource::Library { uid } => {
                Some(Backend::Server(self.client.clone()?, uid.clone()))
            }
            ReaderSource::LocalFile { .. } => Some(Backend::Local(self.local.clone()?)),
        }
    }

    pub fn get(&self, page: u32) -> Option<&PageImg> {
        self.entries.get(&page)
    }

    /// height / width, once the page has been seen.
    pub fn aspect(&self, page: u32) -> Option<f32> {
        self.aspects.get(&page).copied()
    }

    pub fn take_newly_sized(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.newly_sized)
    }

    /// Forget everything (after a re-scan the archive may have changed).
    pub fn reset(&mut self) {
        self.entries.clear();
        self.aspects.clear();
        self.newly_sized.clear();
    }

    /// Start loading every page in `pages` nobody has asked for yet. Idempotent and cheap, so the
    /// view calls it on every render. Does not notify.
    pub fn want(&mut self, pages: RangeInclusive<u32>, center: u32, cx: &mut Context<Self>) {
        self.center = center;
        for page in pages {
            if !self.entries.contains_key(&page) {
                self.fetch(page, cx).detach();
            }
        }
    }

    /// Load `pages` and resolve when all have settled (the ±2 preload before the reader opens).
    pub fn load_window(
        &mut self,
        pages: RangeInclusive<u32>,
        center: u32,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        self.center = center;
        let missing: Vec<u32> = pages.filter(|p| !self.entries.contains_key(p)).collect();
        let tasks: Vec<Task<()>> = missing.into_iter().map(|p| self.fetch(p, cx)).collect();
        cx.spawn(async move |_, _| {
            futures_util::future::join_all(tasks).await;
        })
    }

    pub fn retry(&mut self, page: u32, cx: &mut Context<Self>) {
        self.entries.remove(&page);
        self.fetch(page, cx).detach();
        cx.notify();
    }

    fn fetch(&mut self, page: u32, cx: &mut Context<Self>) -> Task<()> {
        let Some(backend) = self.backend() else {
            return Task::ready(());
        };
        self.entries.insert(page, PageImg::Loading);
        cx.spawn(async move |this, cx| {
            let loaded = runtime::run(async move {
                let _permit = permits().acquire().await.ok()?;
                let bytes = match backend {
                    Backend::Server(client, uid) => client.page_bytes(&uid, page).await.ok()?,
                    Backend::Local(local) => {
                        tokio::task::spawn_blocking(move || local.page_bytes(page))
                            .await
                            .ok()??
                    }
                };
                let format = sniff(&bytes)?;
                let (w, h) = dimensions(&bytes)?;
                Some((format, bytes, w, h))
            })
            .await;
            this.update(cx, |s, cx| {
                match loaded {
                    Some((format, bytes, w, h)) if w > 0 => {
                        let aspect = h as f32 / w as f32;
                        if s.aspects.insert(page, aspect) != Some(aspect) {
                            s.newly_sized.push(page);
                        }
                        s.entries.insert(
                            page,
                            PageImg::Ready(Arc::new(Image::from_bytes(format, bytes))),
                        );
                    }
                    _ => {
                        s.entries.insert(page, PageImg::Failed);
                    }
                }
                s.evict(cx);
                cx.notify();
            })
            .ok();
        })
    }

    /// Drop ready images far from the reading position (and their decoded copies).
    fn evict(&mut self, cx: &mut Context<Self>) {
        let center = self.center;
        let far: Vec<u32> = self
            .entries
            .iter()
            .filter(|(p, img)| matches!(img, PageImg::Ready(_)) && p.abs_diff(center) > KEEP_RADIUS)
            .map(|(p, _)| *p)
            .collect();
        for page in far {
            if let Some(PageImg::Ready(image)) = self.entries.remove(&page) {
                ImageSource::Image(image).remove_asset(cx);
            }
        }
    }
}
