mod backend;
mod keepalive;
mod library_view;
mod remote;
mod saf;
mod vault;

use backend::Backend;
use slint::{ModelRc, SharedString, VecModel};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

slint::include_modules!();

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap())
}

fn show_folders(ui: &App, folders: Vec<String>) {
    let folders: Vec<SharedString> = folders.into_iter().map(Into::into).collect();
    ui.set_folders(ModelRc::new(VecModel::from(folders)));
    ui.set_loading(false);
}

/// Reloads the library and folder list from the server and shows them.
async fn refresh_ui(weak: slint::Weak<App>, backend: Backend, folder_status: String) {
    let loaded = library_view::reload(&weak, &backend).await;
    let folders = backend.folders().await.unwrap_or_default();
    let folder_status = match (&loaded, backend.is_remote()) {
        (Err(e), true) => format!("Server unreachable: {e}. Unlink it in Settings to use the on-device library."),
        _ => folder_status,
    };
    let remote = backend.is_remote();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        show_folders(&ui, folders);
        ui.set_folder_status(folder_status.into());
        ui.set_remote_linked(remote);
    });
}

/// The picker returned a tree: register it, add it as a library folder and rescan.
fn add_picked_folder(weak: slint::Weak<App>, backend: Backend, saf: Arc<saf::Saf>, uri: String) {
    std::thread::spawn(move || {
        let status = match saf.add(&uri) {
            Ok(path) => runtime().block_on(backend.add_folder(&path)).map(|()| format!("Added {path}")),
            Err(e) => Err(e.to_string()),
        };
        let status = status.unwrap_or_else(|e| format!("Could not add folder: {e}"));
        runtime().block_on(refresh_ui(weak, backend, status));
    });
}

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    let base = app.internal_data_path().expect("app data dir");
    // RAR archives that are not real files are copied to the temp dir; the Android default
    // (`/data/local/tmp`) is not writable by apps.
    let cache = base.join("cache");
    let _ = std::fs::create_dir_all(&cache);
    // SAFETY: runs before this process starts any other thread of its own.
    unsafe { std::env::set_var("TMPDIR", &cache) };

    tracing_subscriber::fmt().with_ansi(false).with_max_level(tracing::Level::INFO).init();
    saf::init(&app);
    slint::android::init(app).unwrap();
    let ui = App::new().unwrap();
    ui.set_loading(true);
    ui.set_data_dir(base.to_string_lossy().as_ref().into());
    ui.set_server_status("starting".into());

    let saf = saf::Saf::load(&base);
    let weak = ui.as_weak();
    std::thread::spawn(move || match Backend::start(&base, saf.fs()) {
        Ok(backend) => {
            let ui_weak = weak.clone();
            let picker_backend = backend.clone();
            let server_backend = backend.clone();
            let server_dir = base.clone();
            let linked_url = remote::restore(&base, &backend);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                library_view::register(&ui, server_backend.clone());
                ui.set_remote_url(linked_url.clone().unwrap_or_default().into());
                ui.set_remote_linked(linked_url.is_some());
                let (weak, backend, dir) = (ui_weak.clone(), server_backend.clone(), server_dir.clone());
                ui.on_connect_server(move |url, key| {
                    let (weak, backend, dir) = (weak.clone(), backend.clone(), dir.clone());
                    let (url, key) = (url.to_string(), key.to_string());
                    runtime().spawn(async move {
                        let status = match remote::connect(&dir, &backend, &url, &key).await {
                            Ok(url) => format!("Linked to {url}"),
                            Err(e) => format!("Could not link: {e}"),
                        };
                        refresh_ui(weak, backend, status).await;
                    });
                });
                let (weak, backend, dir) = (ui_weak.clone(), server_backend.clone(), server_dir.clone());
                ui.on_disconnect_server(move || {
                    let (weak, backend, dir) = (weak.clone(), backend.clone(), dir.clone());
                    runtime().spawn(async move {
                        remote::disconnect(&dir, &backend);
                        let _ = weak.upgrade_in_event_loop(|ui| ui.set_remote_url("".into()));
                        let _ = backend.rescan().await;
                        refresh_ui(weak, backend, "Back to the on-device library".into()).await;
                    });
                });
                ui.on_add_folder(move || {
                    let (weak, backend, saf) = (ui_weak.clone(), picker_backend.clone(), saf.clone());
                    let launched = saf::pick_folder(move |picked| {
                        if let Some(uri) = picked {
                            add_picked_folder(weak, backend, saf, uri);
                        }
                    });
                    if let Err(e) = launched {
                        eprintln!("folder picker: {e}");
                    }
                });
            });
            runtime().spawn(keepalive::watch(backend.clone()));
            runtime().spawn(async move {
                let ok = backend.healthy().await;
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_server_status(if ok { "running (in-process)" } else { "unhealthy" }.into());
                });
                // Rescan so folders granted in an earlier run show up (and files added since).
                let _ = backend.rescan().await;
                refresh_ui(weak, backend, String::new()).await;
            });
        }
        Err(e) => {
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_server_status(format!("failed: {e}").into());
                ui.set_loading(false);
            });
        }
    });
    ui.run().unwrap();
}

/// Skia's prebuilt objects reference libc++'s `std::__hash_memory`, which NDK r27's libc++ lacks
/// (NDK r28+ has it; remove this shim when the build moves there).
#[unsafe(no_mangle)]
pub extern "C" fn _ZNSt6__ndk113__hash_memoryEPKvm(p: *const u8, n: usize) -> usize {
    let bytes = unsafe { std::slice::from_raw_parts(p, n) };
    let mut h: usize = 0xcbf29ce484222325;
    for b in bytes {
        h = (h ^ *b as usize).wrapping_mul(0x100000001b3);
    }
    h
}
