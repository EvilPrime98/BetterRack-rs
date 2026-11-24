//! Store and Downloads screens: search the store, start a download, follow and control jobs.
//!
//! On-device downloads land in `<data>/downloads`, which is registered as a library folder on
//! the first download (SAF folders are read-only). With a linked server the job runs there and
//! the server picks the folder.
use crate::backend::Backend;
use crate::{App, JobRow, StoreRow, runtime};
use serde_json::{Value, json};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const DOWNLOAD_DIR: &str = "downloads";

#[derive(Clone)]
struct Post {
    id: i64,
    title: String,
    subtitle: String,
}

#[derive(Clone)]
struct Link {
    uuid: String,
    title: String,
}

#[derive(Default)]
struct State {
    results: Vec<Post>,
    started: HashSet<i64>,
    /// Post waiting for the user to choose one of several links.
    pending: Option<(Post, Vec<Link>)>,
    job_ids: Vec<String>,
    job_states: HashMap<String, String>,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

/// Percent-encodes a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn server_message(body: &Value, fallback: &str) -> String {
    body["message"].as_str().or(body["error"].as_str()).unwrap_or(fallback).to_owned()
}

fn rows(st: &State) -> ModelRc<StoreRow> {
    ModelRc::new(VecModel::from(
        st.results
            .iter()
            .map(|p| StoreRow {
                title: p.title.as_str().into(),
                subtitle: p.subtitle.as_str().into(),
                state: if st.started.contains(&p.id) { "Queued" } else { "" }.into(),
            })
            .collect::<Vec<_>>(),
    ))
}

fn show_results(weak: &Weak<App>, status: String, needs_url: bool) {
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_store_results(rows(&state().lock().unwrap()));
        ui.set_store_status(status.into());
        ui.set_store_needs_url(needs_url);
    });
}

async fn search(weak: Weak<App>, backend: Backend, text: String) {
    let text = text.trim();
    let uri = if text.is_empty() {
        "/api/comics?latest=true&page=1&perPage=20".to_owned()
    } else {
        format!("/api/comics?search={}&page=1&perPage=20", encode(text))
    };
    match backend.send_json("GET", &uri, None).await {
        Ok((status, body)) if status.is_success() => {
            let posts: Vec<Post> = body
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some(Post {
                        id: p["id"].as_i64()?,
                        title: p["title"].as_str()?.to_owned(),
                        subtitle: p["uploadDate"].as_str().unwrap_or_default().chars().take(10).collect(),
                    })
                })
                .collect();
            let empty = posts.is_empty();
            {
                let mut st = state().lock().unwrap();
                st.results = posts;
                st.started.clear();
            }
            show_results(&weak, if empty { "No results.".into() } else { String::new() }, false);
        }
        Ok((status, body)) if status.as_u16() == 409 => {
            show_results(&weak, server_message(&body, "Store is not configured."), true);
        }
        Ok((status, body)) => show_results(&weak, server_message(&body, &format!("Store error: {status}")), false),
        Err(e) => show_results(&weak, format!("Could not reach the store: {e}"), false),
    }
}

/// Makes `<data>/downloads` a library folder (once) so finished comics show up in the library.
async fn ensure_download_library(backend: &Backend, dir: &PathBuf) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.to_string_lossy().into_owned();
    if !backend.folders().await?.contains(&path) {
        backend.add_folder(&path).await?;
    }
    Ok(())
}

fn request_notifications() {
    let _ = crate::saf::with_env(|env| {
        let activity = crate::saf::activity();
        let perm = env.new_string("android.permission.POST_NOTIFICATIONS")?;
        let granted = env
            .call_method(&activity, "checkSelfPermission", "(Ljava/lang/String;)I", &[(&perm).into()])?
            .i()?;
        if granted != 0 {
            let array = env.new_object_array(1, "java/lang/String", &perm)?;
            env.call_method(&activity, "requestPermissions", "([Ljava/lang/String;I)V", &[(&array).into(), 7.into()])?;
        }
        Ok(())
    });
}

async fn start(weak: Weak<App>, backend: Backend, data_dir: PathBuf, post: Post, link: Link) {
    request_notifications();
    let local = !backend.is_remote();
    if local && let Err(e) = ensure_download_library(&backend, &data_dir.join(DOWNLOAD_DIR)).await {
        show_results(&weak, format!("Cannot prepare the downloads folder: {e}"), false);
        return;
    }
    let mut body = json!({ "id": post.id, "title": link.title, "uuid": link.uuid, "strat": "all" });
    if local {
        body["outputDir"] = json!(DOWNLOAD_DIR);
    }
    match backend.send_json("POST", "/api/downloads", Some(&body)).await {
        Ok((status, _)) if status.is_success() => {
            state().lock().unwrap().started.insert(post.id);
            show_results(&weak, String::new(), false);
        }
        Ok((_, body)) => show_results(&weak, server_message(&body, "Download failed."), false),
        Err(e) => show_results(&weak, format!("Download failed: {e}"), false),
    }
}

async fn choose_download(weak: Weak<App>, backend: Backend, data_dir: PathBuf, post: Post) {
    let path = format!("/api/comics/{}/links?strat=all", post.id);
    let links: Vec<Link> = match backend.send_json("GET", &path, None).await {
        Ok((status, body)) if status.is_success() => body["links"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| Some(Link { uuid: l["uuid"].as_str()?.to_owned(), title: l["title"].as_str()?.to_owned() }))
            .collect(),
        Ok((status, body)) => {
            let fallback = if status.as_u16() == 404 { "No download links found." } else { "Failed to fetch links." };
            show_results(&weak, server_message(&body, fallback), false);
            return;
        }
        Err(e) => {
            show_results(&weak, format!("Failed to fetch links: {e}"), false);
            return;
        }
    };
    match links.len() {
        0 => show_results(&weak, "No download links found.".into(), false),
        1 => start(weak, backend, data_dir, post, links.into_iter().next().unwrap()).await,
        _ => {
            let titles: Vec<SharedString> = links.iter().map(|l| l.title.as_str().into()).collect();
            let title = post.title.clone();
            state().lock().unwrap().pending = Some((post, links));
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_store_pick_title(title.into());
                ui.set_store_links(ModelRc::new(VecModel::from(titles)));
            });
        }
    }
}

fn job_detail(job: &Value) -> (String, f32) {
    let p = &job["progress"];
    match (job["state"].as_str().unwrap_or_default(), p["type"].as_str().unwrap_or_default()) {
        ("done", _) => ("Done".into(), 100.0),
        ("queued", _) => ("Queued".into(), 0.0),
        (_, "progress") => (
            format!("{}% · {} / {} MB", p["percent"], p["receivedMB"].as_str().unwrap_or("?"), p["totalMB"].as_str().unwrap_or("?")),
            p["percent"].as_f64().unwrap_or(0.0) as f32,
        ),
        (_, "extracting") => {
            let (done, total) = (p["done"].as_f64().unwrap_or(0.0), p["total"].as_f64().unwrap_or(0.0));
            (format!("Extracting {done} / {total}"), if total > 0.0 { (done / total * 100.0) as f32 } else { 0.0 })
        }
        (_, "retrying") => (format!("Retrying in {}s", p["delaySec"]), 0.0),
        (_, "error") => (p["message"].as_str().unwrap_or("Failed").to_owned(), 100.0),
        (_, "preparing") => ("Preparing".into(), 0.0),
        _ => (String::new(), 0.0),
    }
}

/// Refreshes the job list; returns whether a job just finished (the library has a new comic).
async fn poll_jobs(weak: &Weak<App>, backend: &Backend) -> bool {
    let Ok(body) = backend.get_json("/api/downloads/jobs").await else { return false };
    let jobs = body["jobs"].as_array().cloned().unwrap_or_default();
    let mut finished = false;
    let mut ids = Vec::new();
    let mut shown = Vec::new();
    {
        let mut st = state().lock().unwrap();
        let mut next = HashMap::new();
        for job in &jobs {
            let (Some(id), Some(job_state)) = (job["jobId"].as_str(), job["state"].as_str()) else { continue };
            if job_state == "done" && st.job_states.get(id).is_some_and(|s| s != "done") {
                finished = true;
            }
            next.insert(id.to_owned(), job_state.to_owned());
            let (detail, percent) = job_detail(job);
            ids.push(id.to_owned());
            shown.push(JobRow {
                label: job["label"].as_str().unwrap_or(id).into(),
                state: job_state.into(),
                detail: detail.into(),
                percent,
            });
        }
        st.job_ids = ids;
        st.job_states = next;
    }
    let _ = weak.upgrade_in_event_loop(move |ui| ui.set_download_jobs(ModelRc::new(VecModel::from(shown))));
    finished
}

fn job_id(index: i32) -> Option<String> {
    state().lock().unwrap().job_ids.get(usize::try_from(index).ok()?).cloned()
}

/// Registers the Store and Downloads callbacks and starts the job poller. Runs on the UI thread.
pub fn register<F, Fut>(ui: &App, backend: Backend, data_dir: PathBuf, refresh_ui: F)
where
    F: Fn(Weak<App>, Backend) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_store_search(move |text| {
        runtime().spawn(search(weak.clone(), b.clone(), text.to_string()));
    });

    let (weak, b, dir) = (ui.as_weak(), backend.clone(), data_dir.clone());
    ui.on_store_download(move |index| {
        let post = usize::try_from(index).ok().and_then(|i| state().lock().unwrap().results.get(i).cloned());
        if let Some(post) = post {
            runtime().spawn(choose_download(weak.clone(), b.clone(), dir.clone(), post));
        }
    });

    let (weak, b, dir) = (ui.as_weak(), backend.clone(), data_dir);
    ui.on_store_pick_link(move |index| {
        let picked = state().lock().unwrap().pending.take().and_then(|(post, links)| {
            let link = links.get(usize::try_from(index).ok()?)?.clone();
            Some((post, link))
        });
        if let Some(ui) = weak.upgrade() {
            ui.set_store_links(ModelRc::default());
        }
        if let Some((post, link)) = picked {
            runtime().spawn(start(weak.clone(), b.clone(), dir.clone(), post, link));
        }
    });

    let weak = ui.as_weak();
    ui.on_store_cancel_pick(move || {
        state().lock().unwrap().pending = None;
        if let Some(ui) = weak.upgrade() {
            ui.set_store_links(ModelRc::default());
        }
    });

    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_store_save_url(move |url| {
        let (weak, b) = (weak.clone(), b.clone());
        let url = url.to_string();
        runtime().spawn(async move {
            match b.send_json("PUT", "/api/settings", Some(&json!({ "apiUrl": url.trim() }))).await {
                Ok((status, _)) if status.is_success() => search(weak, b, String::new()).await,
                Ok((status, _)) => show_results(&weak, format!("Could not save: {status}"), true),
                Err(e) => show_results(&weak, format!("Could not save: {e}"), true),
            }
        });
    });

    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_download_retry(move |index| {
        if let Some(id) = job_id(index) {
            let (weak, b) = (weak.clone(), b.clone());
            runtime().spawn(async move {
                let _ = b.send_json("POST", &format!("/api/downloads/{id}/retry"), None).await;
                poll_jobs(&weak, &b).await;
            });
        }
    });
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_download_stop(move |index| {
        if let Some(id) = job_id(index) {
            let (weak, b) = (weak.clone(), b.clone());
            runtime().spawn(async move {
                let _ = b.send_json("DELETE", &format!("/api/downloads/{id}"), None).await;
                poll_jobs(&weak, &b).await;
            });
        }
    });

    let weak = ui.as_weak();
    runtime().spawn(async move {
        if let Ok(settings) = backend.get_json("/api/settings").await {
            let url = settings["apiUrl"].as_str().unwrap_or_default().to_owned();
            let needs = url.is_empty();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_store_url(url.into());
                ui.set_store_needs_url(needs);
            });
        }
        loop {
            if poll_jobs(&weak, &backend).await && !backend.is_remote() {
                let _ = backend.rescan().await;
                refresh_ui(weak.clone(), backend.clone()).await;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
