//! App state (MIGRATION.md §4). Each zustand store becomes an entity or a plain struct.

pub mod comic_cache;
pub mod directories;
pub mod downloads;
pub mod identify;
pub mod library;
pub mod prefs;
pub mod reader_images;
pub mod store;
pub mod thumbnails;

use gpui::{App, AppContext as _, Entity, Global};

use crate::api::ApiClient;

/// Handles to the shared entities. Stored as a gpui `Global` so cards and modals can reach them
/// without threading them through every constructor.
#[derive(Clone)]
pub struct Stores {
    pub library: Entity<library::LibraryStore>,
    pub comics: Entity<comic_cache::ComicCacheStore>,
    pub thumbs: Entity<thumbnails::Thumbnails>,
    pub identify: Entity<identify::IdentifyStore>,
    pub settings: Entity<SettingsStore>,
    pub prefs: Entity<prefs::PrefsStore>,
    pub store: Entity<store::StoreState>,
    pub downloads: Entity<downloads::DownloadsStore>,
    pub directories: Entity<directories::DirectoriesStore>,
}

impl Global for Stores {}

impl Stores {
    pub fn new(cx: &mut App) -> Self {
        Self {
            library: cx.new(|_| library::LibraryStore::default()),
            comics: cx.new(|_| comic_cache::ComicCacheStore::default()),
            thumbs: cx.new(|_| thumbnails::Thumbnails::default()),
            identify: cx.new(|_| identify::IdentifyStore::default()),
            settings: cx.new(|_| SettingsStore::default()),
            prefs: cx.new(|_| prefs::PrefsStore::load()),
            store: cx.new(|_| store::StoreState::default()),
            downloads: cx.new(|_| downloads::DownloadsStore::default()),
            directories: cx.new(|_| directories::DirectoriesStore::default()),
        }
    }

    pub fn set_client(&self, client: ApiClient, cx: &mut App) {
        self.library.update(cx, |s, _| s.client = Some(client.clone()));
        self.comics.update(cx, |s, _| s.client = Some(client.clone()));
        self.thumbs.update(cx, |s, _| s.client = Some(client.clone()));
        self.identify.update(cx, |s, _| s.client = Some(client.clone()));
        self.settings.update(cx, |s, _| s.client = Some(client.clone()));
        self.store.update(cx, |s, _| s.client = Some(client.clone()));
        self.downloads.update(cx, |s, _| s.client = Some(client.clone()));
        self.directories.update(cx, |s, _| s.client = Some(client.clone()));
    }
}

/// `settings.store.ts`: the server returns the full object after each mutation, so replace it.
#[derive(Default)]
pub struct SettingsStore {
    pub client: Option<ApiClient>,
    pub settings: crate::model::AppSettings,
    pub loaded: bool,
}

impl SettingsStore {
    /// Run a call that answers with the full settings, replace ours, and report the error text.
    fn mutate<F, Fut>(&mut self, cx: &mut gpui::Context<Self>, call: F) -> gpui::Task<Result<(), String>>
    where
        F: FnOnce(ApiClient) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = crate::api::ApiResult<crate::model::AppSettings>> + Send + 'static,
    {
        let Some(client) = self.client.clone() else {
            return gpui::Task::ready(Err("The server is not running.".into()));
        };
        cx.spawn(async move |this, cx| {
            let settings = crate::runtime::run(call(client)).await.map_err(|e| e.to_string())?;
            // Library folders and the download folder feed the directory list (gotcha #6).
            directories::invalidate();
            this.update(cx, |s, cx| {
                s.settings = settings;
                s.loaded = true;
                cx.notify();
            })
            .map_err(|e| e.to_string())
        })
    }

    /// `PUT /api/settings` (never carries `outputDirs`).
    pub fn update(&mut self, update: crate::model::SettingsUpdate, cx: &mut gpui::Context<Self>) -> gpui::Task<Result<(), String>> {
        self.mutate(cx, move |c| async move { c.update_settings(&update).await })
    }

    pub fn add_library_folder(&mut self, path: String, cx: &mut gpui::Context<Self>) -> gpui::Task<Result<(), String>> {
        self.mutate(cx, move |c| async move { c.add_library_folder(&path).await })
    }

    pub fn remove_library_folder(&mut self, path: String, cx: &mut gpui::Context<Self>) -> gpui::Task<Result<(), String>> {
        self.mutate(cx, move |c| async move { c.remove_library_folder(&path).await })
    }
}
