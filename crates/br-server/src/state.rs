use br_core::archive::Archives;
use br_core::comic_data::ComicDataStore;
use br_core::download::{Downloader, RetryOpts};
use br_core::downloads::DownloadService;
use br_core::pack::PackExtractor;
use br_core::pixeldrain::{self, PixelDrain};
use br_core::rotating_fetch::RotatingFetch;
use br_core::store::{StoreApi, StoreTiming};
use br_core::config::Config;
use br_core::jobs::JobStore;
use br_core::library::Library;
use br_core::settings::Preferences;
use br_core::thumbnail::{CACHE_DIR_NAME, Thumbnails};
use br_core::wiki::{NoWiki, WikiLookup};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub prefs: Arc<Preferences>,
    pub comic_data: Arc<ComicDataStore>,
    pub jobs: Arc<JobStore>,
    pub archives: Arc<Archives>,
    pub library: Arc<Library>,
    pub thumbnails: Arc<Thumbnails>,
    pub store: Arc<StoreApi>,
    pub downloads: Arc<DownloadService>,
    pub wiki: Arc<dyn WikiLookup>,
}

/// Network behaviour of the store client and the download engine. The defaults are the
/// production values; tests point PixelDrain at a local mock and zero the delays.
#[derive(Clone)]
pub struct NetOptions {
    pub store_timing: StoreTiming,
    pub download_retry: RetryOpts,
    /// `(retries, backoff_ms, jitter_ms)` of the rotating fetch.
    pub rotating: Option<(u32, u64, u64)>,
    /// `(host with port, scheme)` standing in for pixeldrain.com.
    pub pixeldrain: Option<(String, String)>,
}

impl Default for NetOptions {
    fn default() -> Self {
        Self { store_timing: StoreTiming::default(), download_retry: RetryOpts::default(), rotating: None, pixeldrain: None }
    }
}

impl AppState {
    /// Open the three SQLite files under `config.db_dir()`. Wiki lookups are off (`NoWiki`); the
    /// binary uses `open_with_wiki` with `wiki::LiveWiki`.
    pub fn open(config: Config) -> br_core::Result<Self> {
        Self::open_with_wiki(config, Arc::new(NoWiki))
    }

    pub fn open_with_wiki(config: Config, wiki: Arc<dyn WikiLookup>) -> br_core::Result<Self> {
        Self::open_with(config, wiki, NetOptions::default())
    }

    pub fn open_with(config: Config, wiki: Arc<dyn WikiLookup>, net: NetOptions) -> br_core::Result<Self> {
        let prefs = Arc::new(Preferences::open(&config.preferences_db())?);
        let comic_data = Arc::new(ComicDataStore::open(&config.comic_data_db())?);
        let jobs = JobStore::open(&config.jobs_db())?;
        let archives = Arc::new(Archives::new(config.seven_zip_path.clone()));
        let cwd = config.data_dir.to_string_lossy().into_owned();
        let library = Library::new(prefs.clone(), comic_data.clone(), archives.clone(), wiki.clone(), cwd.clone());
        let thumbnails = Thumbnails::new(config.data_dir.join(CACHE_DIR_NAME), archives.clone());

        let client = reqwest::Client::new();
        let mut rotating = RotatingFetch::new(client.clone());
        if let Some((retries, backoff, jitter)) = net.rotating {
            rotating = rotating.with_timing(retries, backoff, jitter);
        }
        let (pd_host, pd_scheme) = net.pixeldrain.clone().unwrap_or_else(|| (pixeldrain::HOST.to_string(), "https".to_string()));
        let pixel = Arc::new(PixelDrain::for_host(rotating.clone(), &pd_host, &pd_scheme));
        let store = Arc::new(StoreApi::new(prefs.clone(), client.clone(), pixel, net.store_timing));
        let pack = Arc::new(PackExtractor::new(archives.clone()));
        let pd_hostname = pd_host.split(':').next().unwrap_or(&pd_host).to_string();
        let downloader = Arc::new(Downloader::new(client, rotating, Some(pack), net.download_retry, &pd_hostname));
        let jobs = Arc::new(jobs);
        let library = Arc::new(library);
        let downloads = Arc::new(DownloadService::new(jobs.clone(), store.clone(), downloader, library.clone(), cwd));
        Ok(Self {
            config: Arc::new(config),
            prefs,
            comic_data,
            jobs,
            archives,
            library,
            thumbnails: Arc::new(thumbnails),
            store,
            downloads,
            wiki,
        })
    }

    /// Re-scan the library folders into the index (blocking; call from `spawn_blocking`).
    pub fn rescan_library(&self) -> br_core::Result<()> {
        self.library.rescan()
    }

    /// Like `scanInBackground`: the scan runs on a blocking thread, requests that read the library
    /// wait for it. The scan is registered before this returns.
    pub fn spawn_rescan(&self) {
        let ticket = self.library.ticket();
        let library = self.library.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = library.rescan_with(ticket) {
                tracing::error!(err = %e, "background library scan failed");
            }
        });
    }
}

/// Run blocking core work (SQLite, filesystem) off the async runtime.
pub async fn blocking<T, F>(f: F) -> Result<T, crate::error::ApiError>
where
    F: FnOnce() -> br_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r.map_err(Into::into),
        Err(e) => Err(crate::error::ApiError::Internal(e.to_string())),
    }
}
