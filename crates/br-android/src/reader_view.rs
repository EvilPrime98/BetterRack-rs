//! The page reader: opens an issue, shows one page at a time, prefetches the neighbours and
//! saves progress (`currentPage`, `readPer`, `read`) through `/api/comic-data`, the same record
//! the desktop and web readers write.
use crate::backend::Backend;
use crate::{App, library_view, runtime};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, Weak};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Longest side of a decoded page; keeps a page under 16 MB of RGBA.
const MAX_SIDE: u32 = 2048;

#[derive(Default)]
struct State {
    uid: String,
    total: u32,
    current: u32,
    rtl: bool,
    /// Bumped on every open and close so late results from an earlier issue are dropped.
    generation: u64,
    pages: HashMap<u32, SharedPixelBuffer<Rgba8Pixel>>,
    inflight: Vec<u32>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

/// Resets the state for a new issue (or none), keeping the reading direction.
fn reset(uid: String) {
    let mut st = state().lock().unwrap();
    let (rtl, generation) = (st.rtl, st.generation + 1);
    *st = State { uid, rtl, generation, ..State::default() };
}

fn decode(bytes: &[u8]) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let img = if img.width().max(img.height()) > MAX_SIDE { img.thumbnail(MAX_SIDE, MAX_SIDE) } else { img };
    let img = img.to_rgba8();
    let (w, h) = img.dimensions();
    Ok(SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h))
}

/// `readPer = round2(page / total * 100)`, `read` once on the last page (same as desktop).
fn progress(page: u32, total: u32) -> (f64, bool) {
    let total = total.max(1);
    ((f64::from(page) / f64::from(total) * 10000.0).round() / 100.0, page >= total)
}

fn save_progress(backend: Backend, uid: String, page: u32, total: u32) {
    let (per, read) = progress(page, total);
    let body = serde_json::json!({ "currentPage": page, "readPer": per, "read": read });
    runtime().spawn(async move {
        if let Err(e) = backend.patch_json(&format!("/api/comic-data/{uid}"), &body).await {
            tracing::warn!("could not save reading progress: {e}");
        }
    });
}

async fn fetch_page(backend: &Backend, uid: &str, page: u32) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
    let bytes = backend.get_bytes(&format!("/read/{uid}/pages/{page}")).await?;
    tokio::task::spawn_blocking(move || decode(&bytes)).await.map_err(|e| e.to_string())?
}

/// Fetches `page` unless cached or already loading. `show` = display it when it arrives.
fn load(weak: Weak<App>, backend: Backend, page: u32, show: bool) {
    let (uid, generation) = {
        let mut st = state().lock().unwrap();
        if page < 1 || page > st.total || st.pages.contains_key(&page) || st.inflight.contains(&page) {
            return;
        }
        st.inflight.push(page);
        (st.uid.clone(), st.generation)
    };
    runtime().spawn(async move {
        let result = fetch_page(&backend, &uid, page).await;
        let shown = {
            let mut st = state().lock().unwrap();
            if st.generation != generation {
                return;
            }
            st.inflight.retain(|&p| p != page);
            if let Ok(buf) = &result {
                st.pages.insert(page, buf.clone());
                let keep = st.current.saturating_sub(2)..=st.current + 2;
                st.pages.retain(|p, _| keep.contains(p));
            }
            show && st.current == page
        };
        if shown {
            let _ = weak.upgrade_in_event_loop(move |ui| match result {
                Ok(buf) => {
                    ui.set_reader_page(Image::from_rgba8(buf));
                    ui.set_reader_has_page(true);
                }
                Err(e) => {
                    ui.set_reader_has_page(false);
                    ui.set_reader_message(format!("Could not read page {page}: {e}").into());
                }
            });
        }
    });
}

/// Makes `page` current: shows it (now if decoded, else when it loads) and prefetches around it.
fn go_to(weak: &Weak<App>, backend: &Backend, page: u32) {
    let (uid, page, total, cached) = {
        let mut st = state().lock().unwrap();
        let page = page.clamp(1, st.total.max(1));
        st.current = page;
        (st.uid.clone(), page, st.total, st.pages.get(&page).cloned())
    };
    let have = cached.is_some();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_reader_current(i32::try_from(page).unwrap_or(1));
        match cached {
            Some(buf) => {
                ui.set_reader_page(Image::from_rgba8(buf));
                ui.set_reader_has_page(true);
            }
            None => {
                ui.set_reader_has_page(false);
                ui.set_reader_message("Loading...".into());
            }
        }
    });
    if !have {
        load(weak.clone(), backend.clone(), page, true);
    }
    load(weak.clone(), backend.clone(), page + 1, false);
    load(weak.clone(), backend.clone(), page.saturating_sub(1), false);
    save_progress(backend.clone(), uid, page, total);
}

async fn open(weak: Weak<App>, backend: Backend, uid: String) {
    let listing = backend.get_json(&format!("/read/{uid}")).await;
    let total = listing.as_ref().ok().and_then(|v| v["totalPages"].as_u64()).unwrap_or(0) as u32;
    if total == 0 {
        let msg = match listing {
            Err(e) => format!("Could not open this comic: {e}"),
            Ok(v) => v["message"].as_str().unwrap_or("No pages found").to_owned(),
        };
        let _ = weak.upgrade_in_event_loop(move |ui| ui.set_reader_message(msg.into()));
        return;
    }
    let saved = backend
        .get_json("/api/comic-data")
        .await
        .ok()
        .and_then(|all| all[uid.as_str()]["currentPage"].as_u64())
        .unwrap_or(1) as u32;
    // A finished comic reopens on its first page.
    let start = if saved >= total { 1 } else { saved.max(1) };
    {
        let mut st = state().lock().unwrap();
        if st.uid != uid {
            return;
        }
        st.total = total;
    }
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_reader_total(i32::try_from(total).unwrap_or(0)));
    go_to(&weak, &backend, start);
}

/// Opens the reader on the comic with this uid, at its saved page. Runs on the UI thread.
pub fn start(ui: &App, backend: &Backend, uid: String) {
    reset(uid.clone());
    ui.set_reader_total(0);
    ui.set_reader_current(1);
    ui.set_reader_has_page(false);
    ui.set_reader_message("Opening...".into());
    ui.set_reader_rtl(state().lock().unwrap().rtl);
    ui.set_reader_open(true);
    runtime().spawn(open(ui.as_weak(), backend.clone(), uid));
}

pub fn register(ui: &App, backend: Backend) {
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_open_issue(move |index| {
        if let Some(uid) = library_view::issue_uid(index) {
            if let Some(ui) = weak.upgrade() {
                start(&ui, &b, uid);
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_reader_close(move || {
        reset(String::new());
        if let Some(ui) = weak.upgrade() {
            ui.set_reader_open(false);
            ui.set_reader_page(Image::default());
        }
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_reader_step(move |delta| {
        let (current, total) = {
            let st = state().lock().unwrap();
            (st.current, st.total)
        };
        let target = i64::from(current) + i64::from(delta);
        if total > 0 && (1..=i64::from(total)).contains(&target) {
            go_to(&weak, &b, target as u32);
        }
    });
    let (weak, b) = (ui.as_weak(), backend);
    ui.on_reader_seek(move |page| {
        if state().lock().unwrap().total > 0 {
            go_to(&weak, &b, u32::try_from(page).unwrap_or(1));
        }
    });
    let weak = ui.as_weak();
    ui.on_reader_toggle_direction(move || {
        let rtl = {
            let mut st = state().lock().unwrap();
            st.rtl = !st.rtl;
            st.rtl
        };
        if let Some(ui) = weak.upgrade() {
            ui.set_reader_rtl(rtl);
        }
    });
}
