//! Browse screen: "Keep reading" and "Recently added" shelves, plus a manual library refresh.
use crate::backend::Backend;
use crate::{App, Item, library_view, reader_view, runtime};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct Row {
    uid: String,
    title: String,
    subtitle: String,
}

#[derive(Default)]
struct State {
    reading: Vec<Row>,
    recent: Vec<Row>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn parse(list: &serde_json::Value, subtitle: impl Fn(&serde_json::Value) -> String) -> Vec<Row> {
    list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(Row { uid: e["uid"].as_str()?.to_owned(), title: e["name"].as_str().unwrap_or_default().to_owned(), subtitle: subtitle(e) })
        })
        .collect()
}

fn issue_label(e: &serde_json::Value) -> String {
    e["comic"]["issue"].as_str().filter(|i| !i.is_empty()).map(|i| format!("Issue {i}")).unwrap_or_default()
}

fn model(rows: &[Row]) -> ModelRc<Item> {
    ModelRc::new(VecModel::from(
        rows.iter().map(|r| library_view::cover_item(&r.title, r.subtitle.clone(), &r.uid)).collect::<Vec<_>>(),
    ))
}

/// Fetches both shelves from the (local or linked) server and shows them.
pub async fn reload(weak: &Weak<App>, backend: &Backend) {
    let progress = backend.get_json("/api/comic-data").await.unwrap_or_default();
    let percent: HashMap<String, f64> = progress
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(uid, r)| Some((uid.clone(), r["readPer"].as_f64()?)))
        .collect();
    let reading = backend.get_json("/api/library/reading").await.map(|v| {
        parse(&v, |e| {
            let done = e["uid"].as_str().and_then(|u| percent.get(u)).map_or(String::new(), |p| format!("{}%", p.round()));
            [issue_label(e), done].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
        })
    });
    let recent = backend.get_json("/api/library/recent").await.map(|v| parse(&v, issue_label));
    {
        let mut st = state().lock().unwrap();
        st.reading = reading.unwrap_or_default();
        st.recent = recent.unwrap_or_default();
    }
    let _ = weak.upgrade_in_event_loop(|ui| apply(&ui));
}

fn apply(ui: &App) {
    let st = state().lock().unwrap();
    ui.set_reading(model(&st.reading));
    ui.set_recent(model(&st.recent));
}

fn row_uid(section: i32, index: i32) -> Option<String> {
    let st = state().lock().unwrap();
    let list = if section == 0 { &st.reading } else { &st.recent };
    list.get(usize::try_from(index).ok()?).map(|r| r.uid.clone())
}

/// Registers the Browse callbacks. `refresh_ui` redraws the whole app after a rescan.
pub fn register<F, Fut>(ui: &App, backend: Backend, refresh_ui: F)
where
    F: Fn(Weak<App>, Backend) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_list_cover_needed(move |section, index| {
        if let Some(uid) = row_uid(section, index) {
            library_view::ensure_cover(weak.clone(), b.clone(), uid);
        }
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_open_list_item(move |section, index| {
        if let (Some(uid), Some(ui)) = (row_uid(section, index), weak.upgrade()) {
            reader_view::start(&ui, &b, uid);
        }
    });
    let weak = ui.as_weak();
    ui.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_refreshing(true);
        }
        let (weak, backend, refresh_ui) = (weak.clone(), backend.clone(), refresh_ui.clone());
        runtime().spawn(async move {
            let _ = backend.rescan().await;
            refresh_ui(weak.clone(), backend).await;
            let _ = weak.upgrade_in_event_loop(|ui| ui.set_refreshing(false));
        });
    });
}
