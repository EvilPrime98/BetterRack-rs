//! The library model: the folder scan, the
//! `uid -> entry` index, paging (flat, by series), "recently added" and "reading" views, folder and
//! file operations, per-folder publisher preferences and identification.
//!
//! Scan rules: every folder plus files whose extension is in `COMIC_EXTENSIONS`,
//! recursively under each (resolved) library root; unreachable roots are skipped; uid is
//! `uid_from_path(path.resolve(..))`; folders first, then names in natural order.
//!
//! Everything here is blocking (filesystem, SQLite, 7-Zip); the server calls it from
//! `spawn_blocking`.

use crate::archive::Archives;
use crate::comic_data::{ComicDataStore, ComicRecord};
use crate::comic_info::{js_number, to_wiki_comic};
use crate::directories::natural_cmp;
use crate::series::series_of;
use crate::settings::Preferences;
use crate::sync::{Semaphore, SingleFlight};
use crate::uid::{resolve_windows, uid_from_path};
use crate::wiki::WikiLookup;
use crate::{CoreError, Result};
use serde_json::{Map, Number, Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, RwLock};

pub const COMIC_EXTENSIONS: [&str; 4] = ["cbz", "cbr", "cb7", "cbt"];
pub const IDENTIFY_CONCURRENCY: usize = 4;
pub const IDENTIFY_BATCH_SIZE: usize = IDENTIFY_CONCURRENCY;
/// The entries per page when a `GET /api/library` request has no limit value.
pub const DEFAULT_LIBRARY_PAGE_SIZE: i64 = 100;
/// The maximum client-supplied page size.
pub const MAX_LIBRARY_PAGE_SIZE: i64 = 500;
/// The default look-back window, in hours, for the "recently added" view.
pub const RECENT_WINDOW_HOURS: i64 = 24;

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryEntry {
    pub uid: String,
    pub did: bool,
    pub name: String,
    pub path: String,
    pub parent_id: Option<String>,
    /// mtime in ms.
    pub created_at: f64,
    /// Index of the library root the entry was found under.
    pub library_index: usize,
    pub pref_publisher: Option<String>,
    pub pref_inheritance: Option<bool>,
    pub pref_cover: Option<String>,
    /// Tri-state, files only: `None` = not looked up yet, `Some(true)` = identified (see `comic`),
    /// `Some(false)` = looked up, no match.
    pub identified: Option<bool>,
    pub comic: Option<Value>,
    pub meta_source: Option<String>,
}

/// A JS number as JSON: whole values print as `1`, not `1.0`.
pub fn number_value(f: f64) -> Value {
    if f.fract() == 0.0 && f.abs() < 9e15 {
        Value::from(f as i64)
    } else {
        Number::from_f64(f).map_or(Value::Null, Value::Number)
    }
}

impl LibraryEntry {
    /// The `TLibraryEntry` JSON the API serves (`undefined` fields are omitted).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("uid".into(), self.uid.clone().into());
        m.insert("did".into(), self.did.into());
        m.insert("name".into(), self.name.clone().into());
        m.insert("path".into(), self.path.clone().into());
        if let Some(p) = &self.parent_id {
            m.insert("parentId".into(), p.clone().into());
        }
        m.insert("createdAt".into(), number_value(self.created_at));
        if let Some(v) = &self.pref_publisher {
            m.insert("prefPublisher".into(), v.clone().into());
        }
        if let Some(v) = self.pref_inheritance {
            m.insert("prefInheritance".into(), v.into());
        }
        if let Some(v) = &self.pref_cover {
            m.insert("prefCover".into(), v.clone().into());
        }
        if let Some(v) = self.identified {
            m.insert("identified".into(), v.into());
        }
        if let Some(v) = &self.comic {
            m.insert("comic".into(), v.clone());
        }
        if let Some(v) = &self.meta_source {
            m.insert("metaSource".into(), v.clone().into());
        }
        Value::Object(m)
    }

    fn issue(&self) -> Option<&str> {
        self.comic
            .as_ref()
            .and_then(|c| c.get("issue"))
            .and_then(Value::as_str)
    }
}

fn mtime_ms(meta: &std::fs::Metadata) -> f64 {
    let since = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    since.as_secs() as f64 * 1e3 + f64::from(since.subsec_nanos()) / 1e6
}

fn is_comic_name(name: &str) -> bool {
    COMIC_EXTENSIONS.contains(&crate::archive::extension(name).as_str())
}

/// Depth-first walk (symlinked folders are not followed, like `readdir({ recursive })`).
fn walk(dir: &Path, out: &mut Vec<(String, bool, std::path::PathBuf)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        if is_dir || is_comic_name(&name) {
            out.push((name, is_dir, entry.path()));
        }
        if is_dir {
            walk(&entry.path(), out);
        }
    }
}

/// Scan the given library roots (already `outputDirs`, resolved against `cwd` here).
pub fn scan(roots: &[String], cwd: &str) -> Vec<LibraryEntry> {
    let mut found: Vec<(LibraryEntry, String)> = Vec::new(); // (entry, parent path)
    for (library_index, root) in roots.iter().enumerate() {
        let root = resolve_windows(root, cwd);
        if !Path::new(&root).is_dir() {
            tracing::error!(lib_path = %root, "skipping unreachable library folder");
            continue;
        }
        let mut raw = Vec::new();
        walk(Path::new(&root), &mut raw);
        for (name, did, path) in raw {
            let abs = resolve_windows(&path.to_string_lossy(), cwd);
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let parent = path
                .parent()
                .map(|p| resolve_windows(&p.to_string_lossy(), cwd))
                .unwrap_or_default();
            found.push((
                LibraryEntry {
                    uid: uid_from_path(&abs),
                    did,
                    name,
                    path: abs,
                    parent_id: None,
                    created_at: mtime_ms(&meta),
                    library_index,
                    pref_publisher: None,
                    pref_inheritance: None,
                    pref_cover: None,
                    identified: None,
                    comic: None,
                    meta_source: None,
                },
                parent,
            ));
        }
    }
    let uid_by_path: HashMap<String, String> = found
        .iter()
        .map(|(e, _)| (e.path.clone(), e.uid.clone()))
        .collect();
    let mut entries: Vec<LibraryEntry> = found
        .into_iter()
        .map(|(mut e, parent)| {
            e.parent_id = uid_by_path.get(&parent).cloned();
            e
        })
        .collect();
    entries.sort_by(|a, b| {
        b.did
            .cmp(&a.did)
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    entries
}

/// Node's `path.basename` for the Windows flavour (`C:\` has an empty basename).
pub fn basename(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    let name = trimmed.rsplit(['\\', '/']).next().unwrap_or("");
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        String::new()
    } else {
        name.to_string()
    }
}

/// `parsePageOption`: `undefined` unless the text is a finite, non-negative number.
pub fn parse_page_option(raw: Option<&str>) -> Option<i64> {
    let raw = raw.filter(|r| !r.is_empty())?;
    let n = js_number(raw);
    (n.is_finite() && n >= 0.0).then(|| n.trunc() as i64)
}

#[derive(Default)]
struct State {
    roots: Vec<String>,
    entries: Vec<LibraryEntry>,
    by_uid: HashMap<String, usize>,
    /// uid -> index of the library root (`entryLibraryIndex`; the last root wins on overlap).
    lib_of: HashMap<String, usize>,
    /// `resolveInheritance()` output; dropped on every mutation.
    resolved: Option<Arc<Vec<LibraryEntry>>>,
}

struct Group {
    uid: String,
    name: String,
    path: String,
    entries: Vec<usize>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Counts scans in progress so readers can wait for them (`await libModel.ready`).
#[derive(Default)]
struct Gate {
    pending: Mutex<usize>,
    cv: Condvar,
}

/// Held while a scan is queued or running; readers of [`Library::wait_ready`] wait for all of them.
pub struct ScanTicket(Arc<Gate>);

impl Drop for ScanTicket {
    fn drop(&mut self) {
        *lock(&self.0.pending) -= 1;
        self.0.cv.notify_all();
    }
}

pub struct Library {
    gate: Arc<Gate>,
    prefs: Arc<Preferences>,
    comic_data: Arc<ComicDataStore>,
    archives: Arc<Archives>,
    wiki: Arc<dyn WikiLookup>,
    cwd: String,
    state: RwLock<State>,
    identify_flight: SingleFlight<String, LibraryEntry>,
    identify_limiter: Semaphore,
}

impl Library {
    pub fn new(
        prefs: Arc<Preferences>,
        comic_data: Arc<ComicDataStore>,
        archives: Arc<Archives>,
        wiki: Arc<dyn WikiLookup>,
        cwd: String,
    ) -> Self {
        Self {
            gate: Arc::default(),
            prefs,
            comic_data,
            archives,
            wiki,
            cwd,
            state: RwLock::new(State::default()),
            identify_flight: SingleFlight::default(),
            identify_limiter: Semaphore::new(IDENTIFY_CONCURRENCY),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, State> {
        self.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, State> {
        self.state.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Mark a scan as pending before handing it to a background thread, so requests arriving in
    /// between already wait for it. Pass the ticket to [`Library::rescan_with`].
    pub fn ticket(&self) -> ScanTicket {
        *lock(&self.gate.pending) += 1;
        ScanTicket(self.gate.clone())
    }

    /// Block until no scan is queued or running.
    pub fn wait_ready(&self) {
        let mut pending = lock(&self.gate.pending);
        while *pending > 0 {
            pending = self
                .gate
                .cv
                .wait(pending)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Re-scan the library folders from the `outputDirs` setting.
    pub fn rescan(&self) -> Result<()> {
        self.rescan_with(self.ticket())
    }

    pub fn rescan_with(&self, _ticket: ScanTicket) -> Result<()> {
        self.scan_now()
    }

    fn scan_now(&self) -> Result<()> {
        let roots: Vec<String> = self
            .prefs
            .get_app_settings()?
            .output_dirs
            .iter()
            .map(|p| resolve_windows(p, &self.cwd))
            .collect();
        let prefs: HashMap<String, _> = self
            .prefs
            .get_all_library_prefs()?
            .into_iter()
            .map(|p| (p.uid.clone(), p))
            .collect();
        let stored = self.comic_data.get_all()?;

        let mut entries = scan(&roots, &self.cwd);
        for entry in &mut entries {
            if let Some(p) = prefs.get(&entry.uid) {
                entry.pref_publisher = Some(p.pref_publisher.clone());
                entry.pref_inheritance = Some(p.recursive);
                entry.pref_cover = Some(p.pref_cover.clone());
            }
            if entry.did {
                continue;
            }
            // hydrateComicData: identified rows bring their metadata, "looked up, no match" rows
            // only the flag; anything else stays un-looked-up.
            if let Some(existing) = stored.get(&entry.uid).and_then(Value::as_object) {
                match existing.get("identified") {
                    Some(Value::Bool(true)) => {
                        entry.identified = Some(true);
                        entry.comic = existing.get("comic").cloned();
                        entry.meta_source = existing
                            .get("metaSource")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                    }
                    Some(Value::Bool(false)) => entry.identified = Some(false),
                    _ => {}
                }
            }
        }
        let by_uid = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.uid.clone(), i))
            .collect();
        let lib_of = entries
            .iter()
            .map(|e| (e.uid.clone(), e.library_index))
            .collect();
        *self.write() = State {
            roots,
            entries,
            by_uid,
            lib_of,
            resolved: None,
        };
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.read().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of files (not folders) in the index.
    pub fn file_count(&self) -> usize {
        self.read().entries.iter().filter(|e| !e.did).count()
    }

    /// `LibraryModel.get(uid)`: the stored entry, without folder inheritance applied.
    pub fn get(&self, uid: &str) -> Option<LibraryEntry> {
        let s = self.read();
        s.by_uid.get(uid).map(|&i| s.entries[i].clone())
    }

    /// The path only (no metadata clone); the reader hits this on every page request.
    pub fn path_of(&self, uid: &str) -> Option<String> {
        let s = self.read();
        s.by_uid
            .get(uid)
            .map(|&i| s.entries[i].path.clone())
            .filter(|p| !p.is_empty())
    }

    pub fn roots(&self) -> Vec<String> {
        self.read().roots.clone()
    }

    fn mutate(&self, uid: &str, f: impl FnOnce(&mut LibraryEntry)) -> Option<LibraryEntry> {
        let mut s = self.write();
        let &i = s.by_uid.get(uid)?;
        f(&mut s.entries[i]);
        s.resolved = None;
        Some(s.entries[i].clone())
    }

    /// `resolveInheritance`: an entry without its own publisher takes the nearest ancestor's when
    /// that ancestor's preference is recursive.
    fn resolved(&self) -> Arc<Vec<LibraryEntry>> {
        if let Some(r) = &self.read().resolved {
            return r.clone();
        }
        let mut s = self.write();
        if let Some(r) = &s.resolved {
            return r.clone();
        }
        let has_publisher =
            |e: &LibraryEntry| e.pref_publisher.as_deref().is_some_and(|p| !p.is_empty());
        let resolved: Vec<LibraryEntry> = s
            .entries
            .iter()
            .map(|entry| {
                if has_publisher(entry) {
                    return entry.clone();
                }
                let mut parent_id = entry.parent_id.as_deref();
                for _ in 0..s.entries.len() {
                    let Some(parent) = parent_id
                        .and_then(|p| s.by_uid.get(p))
                        .map(|&i| &s.entries[i])
                    else {
                        break;
                    };
                    if parent.pref_inheritance == Some(true) && has_publisher(parent) {
                        let mut copy = entry.clone();
                        copy.pref_publisher = parent.pref_publisher.clone();
                        return copy;
                    }
                    parent_id = parent.parent_id.as_deref();
                }
                entry.clone()
            })
            .collect();
        let resolved = Arc::new(resolved);
        s.resolved = Some(resolved.clone());
        resolved
    }

    /// Per-library `{ uid, name, count }`, no entry payloads.
    pub fn index(&self) -> Value {
        let resolved = self.resolved();
        let s = self.read();
        Value::Array(
            s.roots
                .iter()
                .enumerate()
                .map(|(i, root)| {
                    let count = resolved
                        .iter()
                        .filter(|e| s.lib_of.get(&e.uid) == Some(&i))
                        .count();
                    json!({ "uid": uid_from_path(root), "name": basename(root), "count": count })
                })
                .collect(),
        )
    }

    fn library_groups(&self, resolved: &[LibraryEntry]) -> Vec<Group> {
        let s = self.read();
        s.roots
            .iter()
            .enumerate()
            .map(|(i, root)| Group {
                uid: uid_from_path(root),
                name: basename(root),
                path: root.clone(),
                entries: resolved
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| s.lib_of.get(&e.uid) == Some(&i))
                    .map(|(n, _)| n)
                    .collect(),
            })
            .collect()
    }

    /// A slice of the flat entry list in group order, re-nested into its groups. `limit` and
    /// `offset` count entries, not groups.
    fn paginate(
        resolved: &[LibraryEntry],
        groups: Vec<Group>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Value {
        let total: usize = groups.iter().map(|g| g.entries.len()).sum();
        let limit = limit
            .unwrap_or(DEFAULT_LIBRARY_PAGE_SIZE)
            .clamp(1, MAX_LIBRARY_PAGE_SIZE) as usize;
        let offset = (offset.unwrap_or(0).max(0) as usize).min(total);
        let end = (offset + limit).min(total);

        let mut paged = Vec::new();
        let mut cursor = 0;
        for group in groups {
            let (start, group_end) = (cursor, cursor + group.entries.len());
            cursor = group_end;
            if group_end <= offset || start >= end {
                continue;
            }
            let from = offset.max(start) - start;
            let to = end.min(group_end) - start;
            paged.push(json!({
                "uid": group.uid,
                "name": group.name,
                "path": group.path,
                "entries": group.entries[from..to].iter().map(|&i| resolved[i].to_json()).collect::<Vec<_>>(),
            }));
        }
        json!({ "groups": paged, "total": total, "limit": limit, "offset": offset, "hasMore": end < total })
    }

    pub fn page(&self, limit: Option<i64>, offset: Option<i64>) -> Value {
        let resolved = self.resolved();
        let groups = self.library_groups(&resolved);
        Self::paginate(&resolved, groups, limit, offset)
    }

    pub fn page_by_series(&self, limit: Option<i64>, offset: Option<i64>) -> Value {
        let resolved = self.resolved();
        let mut by_key: HashMap<String, usize> = HashMap::new();
        let mut groups: Vec<Group> = Vec::new();
        for (n, entry) in resolved.iter().enumerate() {
            if entry.did {
                continue;
            }
            let issue = if entry.identified == Some(true) {
                entry.issue()
            } else {
                None
            };
            let series = series_of(&entry.name, issue);
            match by_key.get(&series.key) {
                Some(&g) => groups[g].entries.push(n),
                None => {
                    by_key.insert(series.key.clone(), groups.len());
                    groups.push(Group {
                        uid: uid_from_path(&format!("series:{}", series.key)),
                        name: series.name,
                        path: String::new(),
                        entries: vec![n],
                    });
                }
            }
        }
        groups.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        let sort_key = |i: usize| {
            resolved[i]
                .issue()
                .filter(|s| !s.is_empty())
                .unwrap_or(&resolved[i].name)
                .to_string()
        };
        for group in &mut groups {
            group
                .entries
                .sort_by(|&a, &b| natural_cmp(&sort_key(a), &sort_key(b)));
        }
        Self::paginate(&resolved, groups, limit, offset)
    }

    /// File entries added within the window, newest first.
    pub fn recent(&self, window_hours: Option<i64>, now_ms: f64) -> Value {
        let window_hours = window_hours
            .filter(|h| *h > 0)
            .unwrap_or(RECENT_WINDOW_HOURS);
        let since = now_ms - window_hours as f64 * 3_600_000.0;
        let resolved = self.resolved();
        let mut items: Vec<&LibraryEntry> = resolved
            .iter()
            .filter(|e| !e.did && e.created_at >= since && e.created_at <= now_ms)
            .collect();
        items.sort_by(|a, b| b.created_at.total_cmp(&a.created_at));
        json!({ "items": items.iter().map(|e| e.to_json()).collect::<Vec<_>>(), "windowHours": window_hours, "generatedAt": number_value(now_ms) })
    }

    /// Started, unfinished files, most recently read first (unstamped ones last, by name).
    pub fn reading(&self, now_ms: f64) -> Result<Value> {
        let stored = self.comic_data.get_all()?;
        let record = |uid: &str| stored.get(uid).and_then(Value::as_object);
        let in_progress = |r: Option<&ComicRecord>| {
            let read_per = r
                .and_then(|r| r.get("readPer"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            r.and_then(|r| r.get("read")) != Some(&Value::Bool(true))
                && read_per > 0.0
                && read_per < 100.0
        };
        let last_read = |e: &LibraryEntry| {
            record(&e.uid)
                .and_then(|r| r.get("lastReadAt"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        };
        let resolved = self.resolved();
        let mut items: Vec<&LibraryEntry> = resolved
            .iter()
            .filter(|e| !e.did && in_progress(record(&e.uid)))
            .collect();
        items.sort_by(|a, b| {
            last_read(b)
                .total_cmp(&last_read(a))
                .then_with(|| natural_cmp(&a.name, &b.name))
        });
        Ok(
            json!({ "items": items.iter().map(|e| e.to_json()).collect::<Vec<_>>(), "generatedAt": number_value(now_ms) }),
        )
    }

    pub fn get_preferences(&self, uid: &str) -> Result<Option<crate::settings::LibraryPref>> {
        self.prefs.get_library_pref(uid)
    }

    pub fn update_preferences(
        &self,
        uid: &str,
        publisher: Option<String>,
        recursive: Option<bool>,
        cover: Option<String>,
    ) -> Result<()> {
        let updated = self
            .prefs
            .upsert_library_pref(uid, publisher, recursive, cover)?;
        self.mutate(uid, |e| {
            e.pref_publisher = Some(updated.pref_publisher);
            e.pref_inheritance = Some(updated.recursive);
            e.pref_cover = Some(updated.pref_cover);
        });
        Ok(())
    }

    fn file_entry(&self, uid: &str, missing: &str) -> Result<LibraryEntry> {
        match self.get(uid) {
            Some(e) if e.did => Err(CoreError::Invalid("Target is not a file.".into())),
            Some(e) => Ok(e),
            None => Err(CoreError::Invalid(missing.into())),
        }
    }

    fn apply_stored(&self, uid: &str, stored: &ComicRecord) -> Option<LibraryEntry> {
        self.mutate(uid, |e| {
            e.identified = stored.get("identified").and_then(Value::as_bool);
            e.comic = stored.get("comic").cloned();
            e.meta_source = stored
                .get("metaSource")
                .and_then(Value::as_str)
                .map(str::to_string);
        })
    }

    /// Resolve metadata for one comic on demand: stored result, else `ComicInfo.xml`, else (when
    /// `wikiSearch` is on) the wiki. Failures are logged and leave the entry un-looked-up.
    pub fn identify(&self, uid: &str) -> Result<LibraryEntry> {
        let entry = self.file_entry_for_identify(uid)?;
        if entry.identified.is_some() {
            return Ok(entry);
        }
        if let Some(stored) = self
            .comic_data
            .get_by_uid(uid)?
            .filter(|s| s.contains_key("identified"))
        {
            return Ok(self.apply_stored(uid, &stored).unwrap_or(entry));
        }
        Ok(self
            .identify_flight
            .run(&uid.to_string(), || self.identify_job(uid, entry)))
    }

    fn file_entry_for_identify(&self, uid: &str) -> Result<LibraryEntry> {
        self.get(uid)
            .filter(|e| !e.did)
            .ok_or_else(|| CoreError::Invalid("Comic not found.".into()))
    }

    fn identify_job(&self, uid: &str, entry: LibraryEntry) -> LibraryEntry {
        let _permit = self.identify_limiter.acquire();
        let run = || -> Result<()> {
            let wiki_search = self.prefs.get_app_settings()?.wiki_search;
            let comic_info = self.archives.comic_info(Path::new(&entry.path))?;
            if comic_info.is_none() && wiki_search {
                tracing::debug!(
                    uid,
                    "no usable ComicInfo.xml found; falling back to wiki lookup"
                );
            }
            let found = match &comic_info {
                Some(info) => Some(to_wiki_comic(info)),
                None if wiki_search => self.wiki.get_comic(&entry.name, None)?,
                None => None,
            };
            if let Some(fresh) = self
                .comic_data
                .get_by_uid(uid)?
                .filter(|s| s.contains_key("identified"))
            {
                self.apply_stored(uid, &fresh);
                return Ok(());
            }
            match found {
                Some(found) => {
                    let meta_source = if comic_info.is_some() {
                        "comicinfo"
                    } else {
                        "wiki"
                    };
                    self.store_identified(uid, found, meta_source)?;
                }
                None => {
                    self.comic_data.upsert(
                        uid,
                        json!({ "identified": false }).as_object().expect("object"),
                    )?;
                    self.mutate(uid, |e| e.identified = Some(false));
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            tracing::error!(err = %e, "failed to identify library entry");
        }
        self.get(uid).unwrap_or(entry)
    }

    fn store_identified(&self, uid: &str, comic: Value, meta_source: &str) -> Result<()> {
        let field = |k: &str| comic.get(k).cloned().unwrap_or(Value::Null);
        let partial = json!({
            "prefId": field("pageId"),
            "sourceWiki": field("sourceWiki"),
            "metaSource": meta_source,
            "identified": true,
            "comic": comic,
        });
        self.comic_data
            .upsert(uid, partial.as_object().expect("object"))?;
        self.mutate(uid, |e| {
            e.identified = Some(true);
            e.comic = Some(comic);
            e.meta_source = Some(meta_source.to_string());
        });
        Ok(())
    }

    /// Identify every file, `IDENTIFY_BATCH_SIZE` at a time, reporting `(done, total)`.
    pub fn identify_library(&self, on_progress: &(dyn Fn(usize, usize) + Sync)) -> Result<()> {
        let files: Vec<String> = self
            .read()
            .entries
            .iter()
            .filter(|e| !e.did)
            .map(|e| e.uid.clone())
            .collect();
        let total = files.len();
        on_progress(0, total);
        let done = Mutex::new(0usize);
        let failure: Mutex<Option<CoreError>> = Mutex::new(None);
        for batch in files.chunks(IDENTIFY_BATCH_SIZE) {
            std::thread::scope(|scope| {
                for uid in batch {
                    scope.spawn(|| {
                        if let Err(e) = self.identify(uid) {
                            lock(&failure).get_or_insert(e);
                        }
                        let mut d = lock(&done);
                        *d += 1;
                        on_progress(*d, total);
                    });
                }
            });
            if let Some(e) = lock(&failure).take() {
                return Err(e);
            }
        }
        Ok(())
    }

    /// Clear one entry's stored identification and immediately look it up again.
    pub fn reidentify_file(&self, uid: &str) -> Result<LibraryEntry> {
        self.file_entry_for_identify(uid)?;
        self.comic_data.clear_identification(uid, None)?;
        self.mutate(uid, |e| {
            e.identified = None;
            e.comic = None;
            e.meta_source = None;
        });
        self.identify(uid)
    }

    pub fn unidentify_file(&self, uid: &str) -> Result<()> {
        self.file_entry(uid, "File not found.")?;
        self.comic_data.clear_identification(uid, Some(false))?;
        self.mutate(uid, |e| {
            e.identified = Some(false);
            e.comic = None;
            e.meta_source = None;
        });
        Ok(())
    }

    /// Flag every file for a fresh lookup (progress and ratings stay).
    pub fn reidentify_all(&self) -> Result<()> {
        self.comic_data.reset_identification()?;
        let mut s = self.write();
        for e in s.entries.iter_mut().filter(|e| !e.did) {
            e.identified = None;
            e.comic = None;
        }
        s.resolved = None;
        Ok(())
    }

    /// A manual identify pick.
    pub fn commit_identify(&self, uid: &str, comic: Value) -> Result<()> {
        self.file_entry(uid, "File not found.")?;
        self.store_identified(uid, comic, "wiki")
    }

    pub fn create_folder(&self, name: &str, parent_uid: Option<&str>) -> Result<()> {
        let roots = self.roots();
        let base = match parent_uid.filter(|p| !p.is_empty()) {
            None => roots
                .first()
                .cloned()
                .ok_or_else(|| CoreError::Invalid("No library folder configured.".into()))?,
            Some(parent) => match roots.iter().find(|r| uid_from_path(r) == parent) {
                Some(root) => root.clone(),
                None => {
                    self.get(parent)
                        .ok_or_else(|| CoreError::Invalid("Folder does not exist.".into()))?
                        .path
                }
            },
        };
        std::fs::create_dir_all(resolve_windows(name, &base))?;
        Ok(())
    }

    pub fn move_file(&self, file_uid: &str, target_folder_uid: &str) -> Result<()> {
        let file = self
            .get(file_uid)
            .ok_or_else(|| CoreError::Invalid("File not found.".into()))?;
        let target_path = if target_folder_uid.is_empty() {
            self.roots()
                .first()
                .cloned()
                .ok_or_else(|| CoreError::Invalid("No library folder configured.".into()))?
        } else {
            let target = self
                .get(target_folder_uid)
                .ok_or_else(|| CoreError::Invalid("Target folder not found.".into()))?;
            if !target.did {
                return Err(CoreError::Invalid("Target is not a folder.".into()));
            }
            target.path
        };
        let name = basename(&file.path);
        let new_path = resolve_windows(&name, &target_path);

        if file.did {
            let source = resolve_windows(&file.path, &self.cwd);
            let target = resolve_windows(&target_path, &self.cwd);
            let source_parent = Path::new(&source)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| source.clone());
            if target == source {
                return Err(CoreError::Move(
                    "A folder cannot be moved into itself.".into(),
                ));
            }
            if target == source_parent {
                return Err(CoreError::Move(
                    "The folder is already in that location.".into(),
                ));
            }
            if target.starts_with(&format!("{source}\\")) {
                return Err(CoreError::Move(
                    "A folder cannot be moved into one of its own subfolders.".into(),
                ));
            }
        }
        if Path::new(&new_path).exists() {
            return Err(CoreError::Move(format!(
                "An entry named \"{name}\" already exists in the target location."
            )));
        }
        std::fs::rename(&file.path, &new_path)?;
        self.rescan()
    }

    pub fn delete_folder(&self, uid: &str) -> Result<()> {
        let folder = self
            .get(uid)
            .ok_or_else(|| CoreError::Invalid("Folder not found.".into()))?;
        if !folder.did {
            return Err(CoreError::Invalid("Target is not a folder.".into()));
        }
        ignore_not_found(std::fs::remove_dir_all(&folder.path))?;
        self.rescan()
    }

    pub fn delete_file(&self, uid: &str) -> Result<()> {
        let file = self.file_entry(uid, "File not found.")?;
        ignore_not_found(std::fs::remove_file(&file.path))?;
        self.rescan()
    }
}

fn ignore_not_found(r: std::io::Result<()>) -> Result<()> {
    match r {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::NoWiki;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        _data: tempfile::TempDir,
        root: tempfile::TempDir,
        lib: Library,
        comic_data: Arc<ComicDataStore>,
        prefs: Arc<Preferences>,
    }

    fn fixture_with(wiki: Arc<dyn WikiLookup>) -> Fixture {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let prefs = Arc::new(Preferences::open_in_memory().unwrap());
        let comic_data = Arc::new(ComicDataStore::open_in_memory().unwrap());
        prefs
            .update_app_settings(
                json!({ "outputDirs": [root.path().to_string_lossy()] })
                    .as_object()
                    .unwrap(),
            )
            .unwrap();
        let lib = Library::new(
            prefs.clone(),
            comic_data.clone(),
            Arc::new(Archives::new(None)),
            wiki,
            data.path().to_string_lossy().into_owned(),
        );
        Fixture {
            _data: data,
            root,
            lib,
            comic_data,
            prefs,
        }
    }

    fn fixture() -> Fixture {
        fixture_with(Arc::new(NoWiki))
    }

    impl Fixture {
        fn path(&self, rel: &str) -> String {
            resolve_windows(
                &self.root.path().join(rel).to_string_lossy(),
                &self.root.path().to_string_lossy(),
            )
        }
        fn uid(&self, rel: &str) -> String {
            uid_from_path(&self.path(rel))
        }
        fn touch(&self, rel: &str) {
            let p = self.root.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        fn cbz(&self, rel: &str, entries: &[(&str, &[u8])]) {
            let p = self.root.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut zip = zip::ZipWriter::new(std::fs::File::create(p).unwrap());
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, data) in entries {
                zip.start_file(*name, opts).unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        fn rescan(&self) {
            self.lib.rescan().unwrap();
        }
    }

    fn names(page: &Value) -> Vec<String> {
        page["groups"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|g| {
                g["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["name"].as_str().unwrap().to_string())
            })
            .collect()
    }

    #[test]
    fn scans_folders_and_comic_files_only_with_parents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for f in [
            "Alpha/A 1.cbz",
            "Alpha/A 10.CBR",
            "Alpha/A 2.cbz",
            "Alpha/readme.txt",
            "top.cb7",
            "Beta/Nested/b.cbt",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"x").unwrap();
        }
        std::fs::create_dir_all(root.join("Empty")).unwrap();
        let root_s = root.to_string_lossy().into_owned();
        let entries = scan(&[root_s.clone(), "Z:\\missing".into()], &root_s);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Alpha", "Beta", "Empty", "Nested", "A 1.cbz", "A 2.cbz", "A 10.CBR", "b.cbt",
                "top.cb7"
            ]
        );
        assert!(entries[..4].iter().all(|e| e.did) && entries[4..].iter().all(|e| !e.did));

        let by_name = |n: &str| entries.iter().find(|e| e.name == n).unwrap();
        assert_eq!(
            by_name("A 1.cbz").parent_id.as_deref(),
            Some(by_name("Alpha").uid.as_str())
        );
        assert_eq!(
            by_name("Nested").parent_id.as_deref(),
            Some(by_name("Beta").uid.as_str())
        );
        assert_eq!(by_name("Alpha").parent_id, None);
        assert_eq!(
            by_name("top.cb7").uid,
            uid_from_path(&by_name("top.cb7").path)
        );
    }

    #[test]
    fn rescan_replaces_the_index() {
        let f = fixture();
        f.touch("a.cbz");
        assert!(f.lib.is_empty());
        f.rescan();
        let uid = f.uid("a.cbz");
        assert_eq!(f.lib.get(&uid).unwrap().name, "a.cbz");
        assert_eq!(f.lib.path_of(&uid), Some(f.path("a.cbz")));
        assert!(f.lib.get("nope").is_none());
        std::fs::remove_file(f.root.path().join("a.cbz")).unwrap();
        f.rescan();
        assert!(f.lib.get(&uid).is_none());
    }

    #[test]
    fn basename_and_page_options_follow_node_and_js() {
        assert_eq!(basename("C:\\Comics\\Marvel"), "Marvel");
        assert_eq!(basename("C:\\Comics\\Marvel\\"), "Marvel");
        assert_eq!(basename("C:\\"), "");
        assert_eq!(parse_page_option(Some("25")), Some(25));
        assert_eq!(parse_page_option(Some("2.9")), Some(2));
        assert_eq!(parse_page_option(Some("-1")), None);
        assert_eq!(parse_page_option(Some("abc")), None);
        assert_eq!(parse_page_option(Some("")), None);
        assert_eq!(parse_page_option(None), None);
    }

    #[test]
    fn entries_serialize_like_the_ts_objects() {
        let f = fixture();
        f.touch("Sub/a.cbz");
        f.rescan();
        let page = f.lib.page(None, None);
        let entries = page["groups"][0]["entries"].as_array().unwrap();
        let file = entries.iter().find(|e| e["name"] == "a.cbz").unwrap();
        let keys: Vec<&str> = file
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["uid", "did", "name", "path", "parentId", "createdAt"]
        );
        assert!(file["createdAt"].is_number());
        let folder = entries.iter().find(|e| e["name"] == "Sub").unwrap();
        assert!(folder.get("parentId").is_none());
    }

    #[test]
    fn paging_counts_entries_across_groups() {
        let f = fixture();
        for i in 1..=5 {
            f.touch(&format!("c{i}.cbz"));
        }
        f.rescan();
        let p = f.lib.page(Some(2), Some(1));
        assert_eq!(
            (
                p["total"].as_u64(),
                p["limit"].as_u64(),
                p["offset"].as_u64(),
                p["hasMore"].as_bool()
            ),
            (Some(5), Some(2), Some(1), Some(true))
        );
        assert_eq!(names(&p), ["c2.cbz", "c3.cbz"]);
        let last = f.lib.page(Some(10), Some(4));
        assert_eq!(
            (names(&last), last["hasMore"].as_bool()),
            (vec!["c5.cbz".to_string()], Some(false))
        );
        // Offset past the end clamps; limit is clamped to [1, 500].
        let past = f.lib.page(Some(0), Some(99));
        assert_eq!(
            (
                past["offset"].as_u64(),
                past["limit"].as_u64(),
                past["groups"].as_array().unwrap().len()
            ),
            (Some(5), Some(1), 0)
        );
        assert_eq!(f.lib.page(Some(9999), None)["limit"], 500);
        assert_eq!(f.lib.page(None, None)["limit"], 100);
    }

    #[test]
    fn index_counts_entries_per_library() {
        let f = fixture();
        f.touch("a/1.cbz");
        f.rescan();
        let idx = f.lib.index();
        assert_eq!(idx[0]["count"], 2);
        assert_eq!(idx[0]["uid"], uid_from_path(&f.path("")));
    }

    #[test]
    fn groups_by_series_and_sorts() {
        let f = fixture();
        for n in [
            "Saga 10.cbz",
            "Saga 2.cbz",
            "Batman 1.cbz",
            "Notes/ignored.txt",
        ] {
            f.touch(n);
        }
        f.rescan();
        let page = f.lib.page_by_series(None, None);
        let groups = page["groups"].as_array().unwrap();
        assert_eq!(
            groups
                .iter()
                .map(|g| g["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Batman", "Saga"]
        );
        assert_eq!(
            groups[1]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Saga 2.cbz", "Saga 10.cbz"]
        );
        assert_eq!(groups[1]["path"], "");
        assert_eq!(groups[1]["uid"], uid_from_path("series:saga"));
        assert_eq!(page["total"], 3, "folders are not part of the series view");
    }

    #[test]
    fn recent_window_and_order() {
        let f = fixture();
        f.touch("new.cbz");
        f.touch("old.cbz");
        let old = std::fs::File::options()
            .write(true)
            .open(f.root.path().join("old.cbz"))
            .unwrap();
        old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(48 * 3600))
            .unwrap();
        f.rescan();
        let now = mtime_ms(&std::fs::metadata(f.root.path().join("new.cbz")).unwrap()) + 1000.0;
        let r = f.lib.recent(None, now);
        assert_eq!(
            r["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["new.cbz"]
        );
        assert_eq!(r["windowHours"], 24);
        let wide = f.lib.recent(Some(72), now);
        assert_eq!(wide["items"].as_array().unwrap().len(), 2);
        assert_eq!(wide["items"][0]["name"], "new.cbz");
        assert_eq!(f.lib.recent(Some(0), now)["windowHours"], 24);
    }

    fn put(f: &Fixture, uid: &str, v: Value) {
        f.comic_data.upsert(uid, v.as_object().unwrap()).unwrap();
    }

    #[test]
    fn reading_returns_only_started_unfinished_files_most_recent_first() {
        let f = fixture();
        for n in [
            "started.cbz",
            "finished.cbz",
            "marked-read.cbz",
            "untouched.cbz",
            "unread.cbz",
        ] {
            f.touch(n);
        }
        std::fs::create_dir(f.root.path().join("series")).unwrap();
        put(&f, &f.uid("started.cbz"), json!({"readPer": 40}));
        put(&f, &f.uid("finished.cbz"), json!({"readPer": 100}));
        put(
            &f,
            &f.uid("marked-read.cbz"),
            json!({"readPer": 60, "read": true}),
        );
        put(&f, &f.uid("unread.cbz"), json!({"readPer": 0}));
        put(&f, &f.uid("series"), json!({"readPer": 50}));
        f.rescan();
        let items = f.lib.reading(0.0).unwrap();
        assert_eq!(
            items["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["started.cbz"]
        );
    }

    #[test]
    fn reading_orders_by_last_read_then_name() {
        let f = fixture();
        for n in ["older.cbz", "newer.cbz", "b-legacy.cbz", "a-legacy.cbz"] {
            f.touch(n);
        }
        // `upsert` stamps lastReadAt itself, so write the stamps through SQL-free merge order.
        put(&f, &f.uid("b-legacy.cbz"), json!({"readPer": 10}));
        put(&f, &f.uid("a-legacy.cbz"), json!({"readPer": 10}));
        f.comic_data
            .upsert(
                &f.uid("older.cbz"),
                json!({"readPer": 10, "lastReadAt": 1_000})
                    .as_object()
                    .unwrap(),
            )
            .unwrap();
        f.comic_data
            .upsert(
                &f.uid("newer.cbz"),
                json!({"readPer": 10, "lastReadAt": 2_000})
                    .as_object()
                    .unwrap(),
            )
            .unwrap();
        f.rescan();
        let stored = f.comic_data.get_all().unwrap();
        // Progress changes stamp "now", which is newer than the explicit values: check ordering
        // against whatever was stored rather than assuming the explicit stamps survived.
        let stamp = |n: &str| stored[&f.uid(n)]["lastReadAt"].as_i64().unwrap_or(0);
        let mut expected = ["older.cbz", "newer.cbz", "b-legacy.cbz", "a-legacy.cbz"];
        expected.sort_by(|a, b| stamp(b).cmp(&stamp(a)).then_with(|| a.cmp(b)));
        let got = f.lib.reading(0.0).unwrap();
        assert_eq!(
            got["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["name"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn folder_publisher_preference_is_inherited_when_recursive() {
        let f = fixture();
        f.touch("Marvel/Sub/a.cbz");
        f.touch("Other/b.cbz");
        f.rescan();
        f.lib
            .update_preferences(&f.uid("Marvel"), Some("Marvel".into()), Some(true), None)
            .unwrap();
        f.lib
            .update_preferences(&f.uid("Other"), Some("DC".into()), Some(false), None)
            .unwrap();
        let page = f.lib.page(None, None);
        let find = |n: &str| {
            page["groups"][0]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["name"] == n)
                .unwrap()
                .clone()
        };
        assert_eq!(find("a.cbz")["prefPublisher"], "Marvel");
        assert!(
            find("b.cbz").get("prefPublisher").is_none(),
            "non-recursive preference is not inherited"
        );
        assert_eq!(find("Marvel")["prefInheritance"], true);
        assert_eq!(
            f.lib
                .get_preferences(&f.uid("Marvel"))
                .unwrap()
                .unwrap()
                .pref_publisher,
            "Marvel"
        );
        // A fresh scan reads the preferences back.
        f.rescan();
        assert_eq!(
            f.lib
                .get(&f.uid("Other"))
                .unwrap()
                .pref_publisher
                .as_deref(),
            Some("DC")
        );
    }

    const XML: &[u8] = br#"<ComicInfo><Series>Saga</Series><Title>Chapter One</Title><Number>1</Number></ComicInfo>"#;

    #[test]
    fn scan_hydrates_stored_identification() {
        let f = fixture();
        f.touch("a.cbz");
        f.touch("b.cbz");
        f.touch("c.cbz");
        put(
            &f,
            &f.uid("a.cbz"),
            json!({"identified": true, "comic": {"title": "A"}, "metaSource": "wiki"}),
        );
        put(&f, &f.uid("b.cbz"), json!({"identified": false}));
        f.rescan();
        let a = f.lib.get(&f.uid("a.cbz")).unwrap();
        assert_eq!(
            (a.identified, a.comic, a.meta_source.as_deref()),
            (Some(true), Some(json!({"title": "A"})), Some("wiki"))
        );
        assert_eq!(f.lib.get(&f.uid("b.cbz")).unwrap().identified, Some(false));
        assert_eq!(f.lib.get(&f.uid("c.cbz")).unwrap().identified, None);
    }

    #[test]
    fn identify_uses_comic_info_and_persists() {
        let f = fixture();
        f.cbz("Saga 1.cbz", &[("1.png", b"x"), ("ComicInfo.xml", XML)]);
        f.rescan();
        let uid = f.uid("Saga 1.cbz");
        let e = f.lib.identify(&uid).unwrap();
        assert_eq!(
            (e.identified, e.meta_source.as_deref()),
            (Some(true), Some("comicinfo"))
        );
        assert_eq!(e.comic.as_ref().unwrap()["title"], "Chapter One");
        let stored = f.comic_data.get_by_uid(&uid).unwrap().unwrap();
        assert_eq!(
            (
                stored["identified"].as_bool(),
                stored["metaSource"].as_str()
            ),
            (Some(true), Some("comicinfo"))
        );
        assert_eq!(stored["comic"]["issue"], "1");
        // A second call is served from memory.
        assert_eq!(f.lib.identify(&uid).unwrap(), e);
    }

    #[test]
    fn identify_without_metadata_records_no_match_and_obeys_wiki_search() {
        struct Stub(AtomicUsize);
        impl WikiLookup for Stub {
            fn get_comic(&self, title: &str, _: Option<&str>) -> Result<Option<Value>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(Some(
                    json!({"title": title, "pageId": 77, "sourceWiki": "https://dc.fandom.com", "issue": "5"}),
                ))
            }
        }
        let stub = Arc::new(Stub(AtomicUsize::new(0)));
        let f = fixture_with(stub.clone());
        f.cbz("plain.cbz", &[("1.png", b"x")]);
        f.cbz("plain2.cbz", &[("1.png", b"x")]);
        f.rescan();

        // wikiSearch off: no lookup, recorded as "looked up, no match".
        let off = f.lib.identify(&f.uid("plain.cbz")).unwrap();
        assert_eq!(off.identified, Some(false));
        assert_eq!(stub.0.load(Ordering::SeqCst), 0);
        assert_eq!(
            f.comic_data
                .get_by_uid(&f.uid("plain.cbz"))
                .unwrap()
                .unwrap()["identified"],
            false
        );

        f.prefs
            .update_app_settings(json!({"wikiSearch": true}).as_object().unwrap())
            .unwrap();
        let on = f.lib.identify(&f.uid("plain2.cbz")).unwrap();
        assert_eq!(
            (on.identified, on.meta_source.as_deref()),
            (Some(true), Some("wiki"))
        );
        assert_eq!(stub.0.load(Ordering::SeqCst), 1);
        let stored = f
            .comic_data
            .get_by_uid(&f.uid("plain2.cbz"))
            .unwrap()
            .unwrap();
        assert_eq!(
            (stored["prefId"].as_i64(), stored["sourceWiki"].as_str()),
            (Some(77), Some("https://dc.fandom.com"))
        );
    }

    #[test]
    fn identify_rejects_unknown_uids_and_folders() {
        let f = fixture();
        std::fs::create_dir(f.root.path().join("dir")).unwrap();
        f.rescan();
        assert_eq!(
            f.lib.identify("nope").unwrap_err().to_string(),
            "Comic not found."
        );
        assert_eq!(
            f.lib.identify(&f.uid("dir")).unwrap_err().to_string(),
            "Comic not found."
        );
    }

    #[test]
    fn concurrent_identify_calls_share_one_lookup() {
        struct Slow(AtomicUsize);
        impl WikiLookup for Slow {
            fn get_comic(&self, _: &str, _: Option<&str>) -> Result<Option<Value>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(100));
                Ok(None)
            }
        }
        let slow = Arc::new(Slow(AtomicUsize::new(0)));
        let f = fixture_with(slow.clone());
        f.prefs
            .update_app_settings(json!({"wikiSearch": true}).as_object().unwrap())
            .unwrap();
        f.cbz("x.cbz", &[("1.png", b"x")]);
        f.rescan();
        let uid = f.uid("x.cbz");
        std::thread::scope(|s| {
            for _ in 0..5 {
                s.spawn(|| assert_eq!(f.lib.identify(&uid).unwrap().identified, Some(false)));
            }
        });
        assert_eq!(slow.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reidentify_unidentify_reset_and_commit() {
        let f = fixture();
        f.cbz("Saga 1.cbz", &[("1.png", b"x"), ("ComicInfo.xml", XML)]);
        f.cbz("other.cbz", &[("1.png", b"x")]);
        f.rescan();
        let (saga, other) = (f.uid("Saga 1.cbz"), f.uid("other.cbz"));
        f.lib.identify(&saga).unwrap();

        f.lib.unidentify_file(&saga).unwrap();
        let un = f.lib.get(&saga).unwrap();
        assert_eq!(
            (un.identified, un.comic, un.meta_source),
            (Some(false), None, None)
        );
        let stored = f.comic_data.get_by_uid(&saga).unwrap().unwrap();
        assert_eq!(stored["identified"], false);
        assert!(!stored.contains_key("comic"));

        // Re-identify clears the "no match" and looks the file up again.
        let again = f.lib.reidentify_file(&saga).unwrap();
        assert_eq!(
            (again.identified, again.meta_source.as_deref()),
            (Some(true), Some("comicinfo"))
        );

        f.lib
            .commit_identify(
                &other,
                json!({"title": "Picked", "pageId": 5, "sourceWiki": "dc"}),
            )
            .unwrap();
        let picked = f.lib.get(&other).unwrap();
        assert_eq!(
            (picked.identified, picked.meta_source.as_deref()),
            (Some(true), Some("wiki"))
        );
        assert_eq!(
            f.comic_data.get_by_uid(&other).unwrap().unwrap()["prefId"],
            5
        );

        f.lib.reidentify_all().unwrap();
        assert_eq!(f.lib.get(&saga).unwrap().identified, None);
        assert_eq!(f.lib.get(&other).unwrap().identified, None);
        assert!(
            f.comic_data
                .get_by_uid(&other)
                .unwrap()
                .unwrap()
                .get("identified")
                .is_none()
        );

        assert_eq!(
            f.lib.unidentify_file("nope").unwrap_err().to_string(),
            "File not found."
        );
        assert_eq!(
            f.lib
                .commit_identify(&f.uid(""), json!({}))
                .unwrap_err()
                .to_string(),
            "File not found."
        );
    }

    #[test]
    fn identify_library_reports_progress_for_every_file() {
        let f = fixture();
        for i in 0..9 {
            f.cbz(&format!("c{i}.cbz"), &[("1.png", b"x")]);
        }
        std::fs::create_dir(f.root.path().join("dir")).unwrap();
        f.rescan();
        let calls = Mutex::new(Vec::new());
        f.lib
            .identify_library(&|done, total| lock(&calls).push((done, total)))
            .unwrap();
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls.first(), Some(&(0, 9)));
        assert_eq!(calls.last(), Some(&(9, 9)));
        assert_eq!(calls.len(), 10);
        assert!(calls.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(f.comic_data.get_all().unwrap().len() == 9);
    }

    fn mkdir(f: &Fixture, rel: &str) {
        std::fs::create_dir_all(f.root.path().join(rel)).unwrap();
    }

    #[test]
    fn move_rejects_bad_folder_moves() {
        let f = fixture();
        mkdir(&f, "parent/child");
        mkdir(&f, "p/movable");
        mkdir(&f, "a/shared");
        mkdir(&f, "b/shared");
        f.rescan();
        let is_move_err = |r: Result<()>| matches!(r, Err(CoreError::Move(_)));
        assert!(
            is_move_err(f.lib.move_file(&f.uid("parent"), &f.uid("parent/child"))),
            "into a descendant"
        );
        assert!(f.root.path().join("parent").exists());
        assert!(
            is_move_err(f.lib.move_file(&f.uid("parent"), &f.uid("parent"))),
            "into itself"
        );
        assert!(
            is_move_err(f.lib.move_file(&f.uid("p/movable"), &f.uid("p"))),
            "already there"
        );
        assert!(
            is_move_err(f.lib.move_file(&f.uid("a/shared"), &f.uid("b"))),
            "name collision"
        );
    }

    #[test]
    fn move_relocates_folders_and_files_and_rescans() {
        let f = fixture();
        mkdir(&f, "src/movable");
        mkdir(&f, "dst");
        f.touch("loose.cbz");
        f.rescan();
        f.lib
            .move_file(&f.uid("src/movable"), &f.uid("dst"))
            .unwrap();
        assert!(
            f.root.path().join("dst/movable").exists()
                && !f.root.path().join("src/movable").exists()
        );
        assert!(f.lib.get(&f.uid("dst/movable")).is_some(), "rescanned");

        f.lib.move_file(&f.uid("loose.cbz"), &f.uid("dst")).unwrap();
        assert!(f.root.path().join("dst/loose.cbz").exists());
        // An empty target uid means the first library root.
        f.lib.move_file(&f.uid("dst/loose.cbz"), "").unwrap();
        assert!(f.root.path().join("loose.cbz").exists());
        assert_eq!(
            f.lib
                .move_file("nope", &f.uid("dst"))
                .unwrap_err()
                .to_string(),
            "File not found."
        );
        assert_eq!(
            f.lib
                .move_file(&f.uid("loose.cbz"), "nope")
                .unwrap_err()
                .to_string(),
            "Target folder not found."
        );
        assert_eq!(
            f.lib
                .move_file(&f.uid("loose.cbz"), &f.uid("loose.cbz"))
                .unwrap_err()
                .to_string(),
            "Target is not a folder."
        );
    }

    #[test]
    fn create_and_delete() {
        let f = fixture();
        f.touch("Keep/a.cbz");
        f.touch("Gone/b.cbz");
        f.rescan();
        f.lib.create_folder("New/Deep", None).unwrap();
        assert!(f.root.path().join("New/Deep").is_dir());
        f.lib
            .create_folder("In Keep", Some(&f.uid("Keep")))
            .unwrap();
        assert!(f.root.path().join("Keep/In Keep").is_dir());
        f.lib
            .create_folder("By Root", Some(&uid_from_path(&f.path(""))))
            .unwrap();
        assert!(f.root.path().join("By Root").is_dir());
        assert_eq!(
            f.lib
                .create_folder("x", Some("nope"))
                .unwrap_err()
                .to_string(),
            "Folder does not exist."
        );

        f.lib.delete_file(&f.uid("Keep/a.cbz")).unwrap();
        assert!(
            !f.root.path().join("Keep/a.cbz").exists() && f.lib.get(&f.uid("Keep/a.cbz")).is_none()
        );
        assert_eq!(
            f.lib.delete_file(&f.uid("Keep")).unwrap_err().to_string(),
            "Target is not a file."
        );
        f.lib.delete_folder(&f.uid("Gone")).unwrap();
        assert!(!f.root.path().join("Gone").exists());
        assert_eq!(
            f.lib
                .delete_folder(&f.uid("Keep/In Keep/../../nope"))
                .unwrap_err()
                .to_string(),
            "Folder not found."
        );
    }
}
