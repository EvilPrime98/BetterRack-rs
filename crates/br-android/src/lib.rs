mod backend;
mod saf;

use backend::Backend;
use slint::{ModelRc, SharedString, VecModel};
use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

slint::include_modules!();

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap())
}

fn show_library(ui: &App, names: Vec<String>, folders: Vec<String>) {
    let names: Vec<SharedString> = names.into_iter().map(Into::into).collect();
    let folders: Vec<SharedString> = folders.into_iter().map(Into::into).collect();
    ui.set_comics(ModelRc::new(VecModel::from(names)));
    ui.set_folders(ModelRc::new(VecModel::from(folders)));
    ui.set_loading(false);
}

/// Reloads the library list and folder list from the server and shows them.
async fn refresh_ui(weak: slint::Weak<App>, backend: Backend, folder_status: String) {
    let names = backend.library_names().await.unwrap_or_default();
    let folders = backend.folders().await.unwrap_or_default();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        show_library(&ui, names, folders);
        ui.set_folder_status(folder_status.into());
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
            let _ = weak.upgrade_in_event_loop(move |ui| {
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
