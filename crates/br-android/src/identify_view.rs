//! Identify sheet (search the wiki, accept a match, re-identify from the file) and the
//! metadata settings (wiki lookup switch, re-identify all).
use crate::backend::Backend;
use crate::{App, WikiRow, library_view, runtime};
use serde_json::{Value, json};
use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct State {
    uid: String,
    results: Vec<Value>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The wiki sends numbers either as JSON numbers or as strings.
fn text_of(v: &Value) -> Option<String> {
    v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
}

fn describe(comic: &Value) -> (String, String) {
    let title = comic["title"].as_str().unwrap_or("Untitled").to_owned();
    let mut parts = Vec::new();
    if let Some(v) = text_of(&comic["volume"]) {
        parts.push(format!("Vol {v}"));
    }
    if let Some(i) = text_of(&comic["issue"]) {
        parts.push(format!("#{i}"));
    }
    if let Some(y) = text_of(&comic["releaseDate"]["releaseYear"]) {
        parts.push(y);
    }
    (title, parts.join(" · "))
}

fn current_text(body: &Value) -> String {
    if body["identified"].as_bool() == Some(true) && body["comic"].is_object() {
        let (title, sub) = describe(&body["comic"]);
        let source = body["metaSource"].as_str().map(|s| format!(" ({s})")).unwrap_or_default();
        format!("Identified as {title} {sub}{source}")
    } else {
        "Not identified.".into()
    }
}

fn message(body: &Value, fallback: &str) -> String {
    body["message"].as_str().or(body["error"].as_str()).unwrap_or(fallback).to_owned()
}

fn set_status(weak: &Weak<App>, status: String) {
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_identify_status(status.into()));
}

fn set_current(weak: &Weak<App>, current: String) {
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_identify_current(current.into()));
}

async fn open(weak: Weak<App>, backend: Backend, uid: String, title: String) {
    {
        let mut st = state().lock().unwrap();
        st.uid = uid.clone();
        st.results.clear();
    }
    let query = title.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_identify_title(title.into());
        ui.set_identify_query(query.into());
        ui.set_identify_results(ModelRc::default());
        ui.set_identify_current("Checking...".into());
        ui.set_identify_status("".into());
        ui.set_identify_open(true);
    });
    match backend.send_json("GET", &format!("/api/library/{uid}/identify"), None).await {
        Ok((s, body)) if s.is_success() => set_current(&weak, current_text(&body)),
        Ok((_, body)) => set_current(&weak, message(&body, "Could not read the metadata.")),
        Err(e) => set_current(&weak, format!("Could not read the metadata: {e}")),
    }
}

async fn search(weak: Weak<App>, backend: Backend, text: String) {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return;
    }
    set_status(&weak, "Searching...".into());
    let uri = format!("/api/wiki/comics?title={}&thumbnailSize=0", encode(&text));
    match backend.send_json("GET", &uri, None).await {
        Ok((s, body)) if s.is_success() => {
            let list = body.as_array().cloned().unwrap_or_default();
            let rows: Vec<WikiRow> = list
                .iter()
                .map(|c| {
                    let (title, subtitle) = describe(c);
                    WikiRow { title: title.into(), subtitle: subtitle.into() }
                })
                .collect();
            let status = if rows.is_empty() { "No matches." } else { "" };
            state().lock().unwrap().results = list;
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_identify_results(ModelRc::new(VecModel::from(rows)));
                ui.set_identify_status(status.into());
            });
        }
        Ok((_, body)) => set_status(&weak, message(&body, "Wiki search failed.")),
        Err(e) => set_status(&weak, format!("Wiki search failed: {e}")),
    }
}

async fn pick<F, Fut>(weak: Weak<App>, backend: Backend, index: usize, refresh_ui: F)
where
    F: Fn(Weak<App>, Backend) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let (uid, comic) = {
        let st = state().lock().unwrap();
        let Some(comic) = st.results.get(index).cloned() else { return };
        (st.uid.clone(), comic)
    };
    set_status(&weak, "Saving...".into());
    match backend.send_json("POST", "/api/library/file/identify", Some(&json!({ "fileUid": uid, "comic": comic }))).await {
        Ok((s, _)) if s.is_success() => {
            let (title, sub) = describe(&comic);
            set_current(&weak, format!("Identified as {title} {sub}"));
            set_status(&weak, "Saved.".into());
            refresh_ui(weak, backend).await;
        }
        Ok((_, body)) => set_status(&weak, message(&body, "Could not save the match.")),
        Err(e) => set_status(&weak, format!("Could not save the match: {e}")),
    }
}

async fn reset<F, Fut>(weak: Weak<App>, backend: Backend, refresh_ui: F)
where
    F: Fn(Weak<App>, Backend) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let uid = state().lock().unwrap().uid.clone();
    set_status(&weak, "Reading the file...".into());
    match backend.send_json("POST", &format!("/api/library/{uid}/identify/reset"), None).await {
        Ok((s, body)) if s.is_success() => {
            set_current(&weak, current_text(&body));
            set_status(&weak, String::new());
            refresh_ui(weak, backend).await;
        }
        Ok((_, body)) => set_status(&weak, message(&body, "Could not re-identify.")),
        Err(e) => set_status(&weak, format!("Could not re-identify: {e}")),
    }
}

fn set_meta_status(weak: &Weak<App>, status: String) {
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_meta_status(status.into()));
}

/// Registers the identify and metadata-settings callbacks. Runs on the UI thread.
pub fn register<F, Fut>(ui: &App, backend: Backend, refresh_ui: F)
where
    F: Fn(Weak<App>, Backend) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_identify_issue(move |index| {
        let Some((uid, title)) = library_view::issue_ref(index) else { return };
        runtime().spawn(open(weak.clone(), b.clone(), uid, title));
    });
    let weak = ui.as_weak();
    ui.on_identify_close(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_identify_open(false);
        }
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_identify_search(move |text| {
        runtime().spawn(search(weak.clone(), b.clone(), text.to_string()));
    });
    let (weak, b, r) = (ui.as_weak(), backend.clone(), refresh_ui.clone());
    ui.on_identify_pick(move |index| {
        if let Ok(i) = usize::try_from(index) {
            runtime().spawn(pick(weak.clone(), b.clone(), i, r.clone()));
        }
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_identify_reset(move || {
        runtime().spawn(reset(weak.clone(), b.clone(), refresh_ui.clone()));
    });

    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_set_wiki_search(move |on| {
        let (weak, b) = (weak.clone(), b.clone());
        runtime().spawn(async move {
            let status = match b.send_json("PUT", "/api/settings", Some(&json!({ "wikiSearch": on }))).await {
                Ok((s, _)) if s.is_success() => String::new(),
                Ok((s, _)) => format!("Could not save: {s}"),
                Err(e) => format!("Could not save: {e}"),
            };
            let _ = weak.upgrade_in_event_loop(move |ui| ui.set_wiki_search(on));
            set_meta_status(&weak, status);
        });
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_reidentify_all(move || {
        let (weak, b) = (weak.clone(), b.clone());
        runtime().spawn(async move {
            let status = match b.send_json("POST", "/api/library/identify/reset-all", None).await {
                Ok((s, _)) if s.is_success() => "Library flagged for re-identification.".to_owned(),
                Ok((s, _)) => format!("Failed: {s}"),
                Err(e) => format!("Failed: {e}"),
            };
            set_meta_status(&weak, status);
        });
    });

    let weak = ui.as_weak();
    runtime().spawn(async move {
        if let Ok(settings) = backend.get_json("/api/settings").await {
            let on = settings["wikiSearch"].as_bool().unwrap_or(false);
            let _ = weak.upgrade_in_event_loop(move |ui| ui.set_wiki_search(on));
        }
    });
}
