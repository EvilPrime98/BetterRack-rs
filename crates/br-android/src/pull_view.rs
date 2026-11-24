//! Pull list: one Wednesday-anchored week of store posts, with the ones that match a followed
//! keyword marked. Tapping a post follows or unfollows its series.
use crate::backend::Backend;
use crate::{App, PullRow, runtime};
use serde_json::json;
use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const PAGE_SIZE: usize = 100;
const MAX_PAGES: u32 = 5;

#[derive(Clone)]
struct Entry {
    id: String,
    keyword: String,
}

#[derive(Clone)]
struct Post {
    id: i64,
    title: String,
    date: String,
}

struct State {
    week: i64,
    entries: Vec<Entry>,
    posts: Vec<Post>,
    /// Bumped on every week change so a late answer for an older week is dropped.
    epoch: u64,
}

fn state() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(State { week: week_start(today()), entries: Vec::new(), posts: Vec::new(), epoch: 0 }))
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The Wednesday on or before `day` (1970-01-01 was a Thursday).
fn week_start(day: i64) -> i64 {
    day - (day + 1).rem_euclid(7)
}

fn today() -> i64 {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    (secs / 86_400) as i64
}

fn day_start(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}T00:00:00")
}

fn short(day: i64) -> String {
    let (_, m, d) = civil_from_days(day);
    format!("{} {}", MONTHS[(m - 1) as usize], d)
}

fn range_label(start: i64) -> String {
    format!("{} - {}", short(start), short(start + 6))
}

fn normalize(s: &str) -> String {
    s.replace('_', " ").to_lowercase()
}

/// A keyword guess from a post title: the title up to the issue number or parenthesis.
fn keyword_from_title(title: &str) -> String {
    let t = title.replace('_', " ");
    let end = t.find(['#', '(']).unwrap_or(t.len());
    t[..end].trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace() || c == '-').trim().to_owned()
}

fn followed_by<'a>(entries: &'a [Entry], title: &str) -> Option<&'a Entry> {
    let name = normalize(&keyword_from_title(title));
    entries.iter().find(|e| !name.is_empty() && normalize(e.keyword.trim()) == name)
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn show(weak: &Weak<App>, status: String) {
    let st = state().lock().unwrap();
    let rows: Vec<PullRow> = st
        .posts
        .iter()
        .map(|p| PullRow {
            title: p.title.as_str().into(),
            subtitle: p.date.as_str().into(),
            followed: followed_by(&st.entries, &p.title).is_some(),
        })
        .collect();
    let label = range_label(st.week);
    let at_current = st.week >= week_start(today());
    let following = st.entries.iter().map(|e| e.keyword.as_str()).collect::<Vec<_>>().join(", ");
    drop(st);
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.set_pull_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_pull_week(label.into());
        ui.set_pull_at_current(at_current);
        ui.set_pull_following(following.into());
        ui.set_pull_status(status.into());
    });
}

async fn load_entries(backend: &Backend) -> Result<(), String> {
    let body = backend.get_json("/api/pull-list").await?;
    let entries = body
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| Some(Entry { id: e["id"].as_str()?.to_owned(), keyword: e["keyword"].as_str()?.to_owned() }))
        .collect();
    state().lock().unwrap().entries = entries;
    Ok(())
}

async fn load_week(weak: Weak<App>, backend: Backend) {
    let (week, epoch) = {
        let mut st = state().lock().unwrap();
        st.epoch += 1;
        st.posts.clear();
        (st.week, st.epoch)
    };
    show(&weak, "Loading...".into());
    if let Err(e) = load_entries(&backend).await {
        show(&weak, format!("Could not load the pull list: {e}"));
        return;
    }
    let (after, before) = (day_start(week), day_start(week + 7));
    let mut all: Vec<Post> = Vec::new();
    for page in 1..=MAX_PAGES {
        let uri = format!("/api/comics?latest=true&after={}&before={}&page={page}&perPage={PAGE_SIZE}", encode(&after), encode(&before));
        let items = match backend.send_json("GET", &uri, None).await {
            Ok((status, body)) if status.is_success() => body,
            Ok((status, body)) if status.as_u16() == 409 => {
                let msg = body["message"].as_str().unwrap_or("Set the store address in the Store tab.");
                show(&weak, msg.to_owned());
                return;
            }
            Ok((status, _)) => {
                show(&weak, format!("Store error: {status}"));
                return;
            }
            Err(e) => {
                show(&weak, format!("Could not reach the store: {e}"));
                return;
            }
        };
        let list = items.as_array().cloned().unwrap_or_default();
        let last = list.len() < PAGE_SIZE;
        for p in &list {
            let (Some(id), Some(title)) = (p["id"].as_i64(), p["title"].as_str()) else { continue };
            if !all.iter().any(|q| q.id == id) {
                all.push(Post { id, title: title.to_owned(), date: p["uploadDate"].as_str().unwrap_or_default().chars().take(10).collect() });
            }
        }
        if last {
            break;
        }
    }
    {
        let mut st = state().lock().unwrap();
        if st.epoch != epoch {
            return;
        }
        st.posts = all;
    }
    let empty = state().lock().unwrap().posts.is_empty();
    show(&weak, if empty { "No posts this week.".into() } else { String::new() });
}

async fn toggle(weak: Weak<App>, backend: Backend, index: usize) {
    let (post, existing) = {
        let st = state().lock().unwrap();
        let Some(post) = st.posts.get(index).cloned() else { return };
        let existing = followed_by(&st.entries, &post.title).cloned();
        (post, existing)
    };
    let result = match existing {
        Some(e) => backend.send_json("DELETE", &format!("/api/pull-list/{}", e.id), None).await,
        None => {
            let keyword = keyword_from_title(&post.title);
            backend.send_json("POST", "/api/pull-list", Some(&json!({ "keyword": keyword }))).await
        }
    };
    let status = match result {
        Ok((s, _)) if s.is_success() => String::new(),
        Ok((s, body)) => body["message"].as_str().map_or_else(|| format!("Could not update: {s}"), str::to_owned),
        Err(e) => format!("Could not update: {e}"),
    };
    let _ = load_entries(&backend).await;
    show(&weak, status);
}

fn move_week(weak: &Weak<App>, backend: &Backend, to: impl FnOnce(i64) -> i64) {
    {
        let mut st = state().lock().unwrap();
        st.week = to(st.week);
    }
    runtime().spawn(load_week(weak.clone(), backend.clone()));
}

/// Registers the pull list callbacks and loads the current week. Runs on the UI thread.
pub fn register(ui: &App, backend: Backend) {
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_pull_prev(move || move_week(&weak, &b, |w| w - 7));
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_pull_next(move || move_week(&weak, &b, |w| (w + 7).min(week_start(today()))));
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_pull_now(move || move_week(&weak, &b, |_| week_start(today())));
    let (weak, b) = (ui.as_weak(), backend.clone());
    ui.on_pull_toggle(move |index| {
        if let Ok(i) = usize::try_from(index) {
            runtime().spawn(toggle(weak.clone(), b.clone(), i));
        }
    });
    let (weak, b) = (ui.as_weak(), backend);
    ui.on_pull_reload(move || {
        runtime().spawn(load_week(weak.clone(), b.clone()));
    });
}
