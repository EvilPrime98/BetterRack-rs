//! Library browsing: series grid, search, filters and series detail.
//!
//! The server groups the library by series (`/api/library/by-series`). This module keeps that
//! list in memory, filters it for the UI and loads cover thumbnails lazily, when Slint creates a
//! row (`cover-needed`).
use crate::backend::Backend;
use crate::{App, Item, runtime};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel, Weak};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

const PAGE: usize = 500;
const FILTER_IDENTIFIED: i32 = 1;
const FILTER_UNIDENTIFIED: i32 = 2;

#[derive(Clone)]
struct Issue {
    uid: String,
    name: String,
    issue: String,
    identified: bool,
}

#[derive(Clone)]
struct Series {
    uid: String,
    name: String,
    issues: Vec<Issue>,
}

#[derive(Default)]
struct State {
    all: Vec<Series>,
    query: String,
    filter: i32,
    /// Indices into `all` that pass the query and the filter, in display order.
    shown: Vec<usize>,
    /// Uid of the series whose detail is open.
    open: Option<String>,
    /// Decoded thumbnails by comic uid; `None` = the server has none.
    covers: HashMap<String, Option<SharedPixelBuffer<Rgba8Pixel>>>,
    inflight: HashSet<String>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn semaphore() -> &'static Arc<Semaphore> {
    static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEM.get_or_init(|| Arc::new(Semaphore::new(4)))
}

fn parse_issue(e: &serde_json::Value) -> Option<Issue> {
    Some(Issue {
        uid: e["uid"].as_str()?.to_owned(),
        name: e["name"].as_str().unwrap_or_default().to_owned(),
        issue: e["comic"]["issue"].as_str().unwrap_or_default().to_owned(),
        identified: e["identified"].as_bool() == Some(true),
    })
}

/// Every page of `/api/library/by-series`. A series cut by a page boundary comes back as two
/// groups with the same uid; they are merged.
async fn fetch(backend: &Backend) -> Result<Vec<Series>, String> {
    let mut all: Vec<Series> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    let mut offset = 0;
    loop {
        let page = backend.get_json(&format!("/api/library/by-series?limit={PAGE}&offset={offset}")).await?;
        for g in page["groups"].as_array().ok_or("library: no groups")? {
            let uid = g["uid"].as_str().unwrap_or_default().to_owned();
            let issues: Vec<Issue> = g["entries"].as_array().into_iter().flatten().filter_map(parse_issue).collect();
            match at.get(&uid) {
                Some(&i) => all[i].issues.extend(issues),
                None => {
                    at.insert(uid.clone(), all.len());
                    all.push(Series { uid, name: g["name"].as_str().unwrap_or_default().to_owned(), issues });
                }
            }
        }
        offset += PAGE;
        if page["hasMore"].as_bool() != Some(true) {
            return Ok(all);
        }
    }
}

fn matches(s: &Series, query: &str, filter: i32) -> bool {
    let kind_ok = match filter {
        FILTER_IDENTIFIED => s.issues.iter().any(|i| i.identified),
        FILTER_UNIDENTIFIED => s.issues.iter().any(|i| !i.identified),
        _ => true,
    };
    let q = query.trim().to_lowercase();
    kind_ok && (q.is_empty() || s.name.to_lowercase().contains(&q) || s.issues.iter().any(|i| i.name.to_lowercase().contains(&q)))
}

fn refilter(st: &mut State) {
    st.shown = (0..st.all.len()).filter(|&i| matches(&st.all[i], &st.query, st.filter)).collect();
}

/// A list row for `uid`, with its cover when already loaded.
pub fn cover_item(title: &str, subtitle: String, uid: &str) -> Item {
    item(&state().lock().unwrap(), title, subtitle, uid)
}

/// Starts loading the cover of `uid` unless it is known or in flight.
pub fn ensure_cover(weak: Weak<App>, backend: Backend, uid: String) {
    let mut st = state().lock().unwrap();
    if st.covers.contains_key(&uid) || !st.inflight.insert(uid.clone()) {
        return;
    }
    drop(st);
    load_cover(weak, backend, uid);
}

fn item(st: &State, title: &str, subtitle: String, uid: &str) -> Item {
    let cover = st.covers.get(uid).and_then(Option::as_ref).map(|buf| Image::from_rgba8(buf.clone()));
    Item {
        title: title.into(),
        subtitle: subtitle.into(),
        uid: uid.into(),
        has_cover: cover.is_some(),
        cover: cover.unwrap_or_default(),
    }
}

fn series_rows(st: &State) -> Vec<Item> {
    st.shown
        .iter()
        .map(|&i| {
            let s = &st.all[i];
            let n = s.issues.len();
            let subtitle = format!("{n} {}", if n == 1 { "issue" } else { "issues" });
            item(st, &s.name, subtitle, s.issues.first().map_or("", |f| &f.uid))
        })
        .collect()
}

fn open_series(st: &State) -> Option<&Series> {
    let uid = st.open.as_ref()?;
    st.all.iter().find(|s| &s.uid == uid)
}

fn issue_rows(st: &State) -> Vec<Item> {
    open_series(st).map_or_else(Vec::new, |s| {
        s.issues
            .iter()
            .map(|i| {
                let subtitle = if i.issue.is_empty() { String::new() } else { format!("Issue {}", i.issue) };
                item(st, &i.name, subtitle, &i.uid)
            })
            .collect()
    })
}

/// Pushes the current state to the UI. Runs on the UI thread.
fn apply(ui: &App) {
    let st = state().lock().unwrap();
    ui.set_series(ModelRc::new(VecModel::from(series_rows(&st))));
    match open_series(&st) {
        Some(s) => {
            ui.set_detail_title(s.name.as_str().into());
            ui.set_issues(ModelRc::new(VecModel::from(issue_rows(&st))));
            ui.set_detail_open(true);
        }
        None => ui.set_detail_open(false),
    }
}

/// Fetches the library from the (local or linked) server and shows it.
pub async fn reload(weak: &Weak<App>, backend: &Backend) -> Result<(), String> {
    let loaded = fetch(backend).await;
    {
        let mut st = state().lock().unwrap();
        st.all = loaded.as_ref().map(Clone::clone).unwrap_or_default();
        if open_series(&st).is_none() {
            st.open = None;
        }
        refilter(&mut st);
    }
    let _ = weak.upgrade_in_event_loop(|ui| apply(&ui));
    loaded.map(|_| ())
}

fn decode(bytes: &[u8]) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let img = image::load_from_memory(bytes).ok()?.thumbnail(320, 480).to_rgba8();
    let (w, h) = img.dimensions();
    Some(SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h))
}

/// Shows `buf` on every row of `model` with this uid.
fn set_cover(model: ModelRc<Item>, uid: &str, buf: &SharedPixelBuffer<Rgba8Pixel>) {
    for i in 0..model.row_count() {
        if let Some(mut row) = model.row_data(i)
            && row.uid.as_str() == uid
        {
            row.cover = Image::from_rgba8(buf.clone());
            row.has_cover = true;
            model.set_row_data(i, row);
        }
    }
}

fn load_cover(weak: Weak<App>, backend: Backend, uid: String) {
    runtime().spawn(async move {
        let _permit = semaphore().acquire().await;
        let buf = match backend.get_bytes(&format!("/api/thumbnail/{uid}")).await {
            Ok(bytes) => tokio::task::spawn_blocking(move || decode(&bytes)).await.ok().flatten(),
            Err(_) => None,
        };
        {
            let mut st = state().lock().unwrap();
            st.inflight.remove(&uid);
            st.covers.insert(uid.clone(), buf.clone());
        }
        if let Some(buf) = buf {
            let _ = weak.upgrade_in_event_loop(move |ui| {
                set_cover(ui.get_series(), &uid, &buf);
                set_cover(ui.get_issues(), &uid, &buf);
                set_cover(ui.get_reading(), &uid, &buf);
                set_cover(ui.get_recent(), &uid, &buf);
            });
        }
    });
}

/// Uid of the issue at `index` in the open series' detail list.
pub fn issue_uid(index: i32) -> Option<String> {
    let st = state().lock().unwrap();
    let index = usize::try_from(index).ok()?;
    open_series(&st)?.issues.get(index).map(|i| i.uid.clone())
}

/// Called when Slint creates a row: start loading its cover unless known or in flight.
fn cover_needed(weak: Weak<App>, backend: Backend, detail: bool, index: i32) {
    let Ok(index) = usize::try_from(index) else { return };
    let st = state().lock().unwrap();
    let uid = if detail {
        open_series(&st).and_then(|s| s.issues.get(index)).map(|i| i.uid.clone())
    } else {
        st.shown.get(index).and_then(|&i| st.all[i].issues.first()).map(|i| i.uid.clone())
    };
    drop(st);
    if let Some(uid) = uid {
        ensure_cover(weak, backend, uid);
    }
}

/// Registers the library callbacks. Runs on the UI thread.
pub fn register(ui: &App, backend: Backend) {
    let weak = ui.as_weak();
    ui.on_search(move |text| {
        let mut st = state().lock().unwrap();
        st.query = text.to_string();
        refilter(&mut st);
        let rows = series_rows(&st);
        drop(st);
        if let Some(ui) = weak.upgrade() {
            ui.set_series(ModelRc::new(VecModel::from(rows)));
        }
    });
    let weak = ui.as_weak();
    ui.on_set_filter(move |filter| {
        let mut st = state().lock().unwrap();
        st.filter = filter;
        refilter(&mut st);
        let rows = series_rows(&st);
        drop(st);
        if let Some(ui) = weak.upgrade() {
            ui.set_filter(filter);
            ui.set_series(ModelRc::new(VecModel::from(rows)));
        }
    });
    let weak = ui.as_weak();
    ui.on_open_series(move |index| {
        let mut st = state().lock().unwrap();
        let Some(&i) = usize::try_from(index).ok().and_then(|n| st.shown.get(n)) else { return };
        st.open = Some(st.all[i].uid.clone());
        drop(st);
        if let Some(ui) = weak.upgrade() {
            apply(&ui);
        }
    });
    let weak = ui.as_weak();
    ui.on_close_series(move || {
        state().lock().unwrap().open = None;
        if let Some(ui) = weak.upgrade() {
            ui.set_detail_open(false);
        }
    });
    let weak = ui.as_weak();
    ui.on_cover_needed(move |detail, index| cover_needed(weak.clone(), backend.clone(), detail, index));
}
