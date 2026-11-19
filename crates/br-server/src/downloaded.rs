use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE_NAME: &str = "downloaded.json";

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub ids: BTreeSet<i64>,
    pub paths: BTreeMap<i64, String>,
}

fn parse(text: &str) -> Option<Snapshot> {
    serde_json::from_str(text).ok().or_else(|| {
        serde_json::from_str::<BTreeSet<i64>>(text)
            .ok()
            .map(|ids| Snapshot {
                ids,
                paths: BTreeMap::new(),
            })
    })
}

pub struct Downloaded {
    file: PathBuf,
    state: Mutex<Snapshot>,
}

impl Downloaded {
    pub fn open(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE_NAME);
        let state = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| parse(&s))
            .unwrap_or_default();
        Self {
            file,
            state: Mutex::new(state),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Snapshot> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> Snapshot {
        self.lock().clone()
    }

    pub fn record(&self, id: i64, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        let mut state = self.lock();
        let new_id = state.ids.insert(id);
        let new_path = state.paths.insert(id, path.clone()).as_deref() != Some(path.as_str());
        if !new_id && !new_path {
            return;
        }
        self.save(&state);
    }

    /// Add the id and set its path only when it has none; an existing path is kept.
    /// Returns whether anything changed. Saves only on change.
    pub fn fill(&self, id: i64, path: &Path) -> bool {
        let mut state = self.lock();
        if !fill_snapshot(&mut state, id, &path.to_string_lossy()) {
            return false;
        }
        self.save(&state);
        true
    }

    fn save(&self, state: &Snapshot) {
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = serde_json::to_string(state)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&self.file, json).map_err(|e| e.to_string()));
        if let Err(e) = written {
            tracing::warn!(
                "could not save downloaded posts to {}: {e}",
                self.file.display()
            );
        }
    }
}

/// The fill-only rule on a snapshot. Returns whether it changed.
fn fill_snapshot(state: &mut Snapshot, id: i64, path: &str) -> bool {
    let new_id = state.ids.insert(id);
    let new_path = !state.paths.contains_key(&id);
    if new_path {
        state.paths.insert(id, path.to_string());
    }
    new_id || new_path
}

pub async fn list(State(s): State<AppState>) -> Json<Snapshot> {
    Json(s.downloaded.snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_id_arrays_and_records_paths() {
        let dir = tempfile_dir();
        std::fs::write(dir.join(FILE_NAME), "[1,2]").unwrap();
        let d = Downloaded::open(&dir);
        assert_eq!(d.snapshot().ids.len(), 2);
        assert!(d.snapshot().paths.is_empty());

        d.record(3, Path::new("/lib/a.cbz"));
        let reopened = Downloaded::open(&dir).snapshot();
        assert_eq!(reopened.ids, BTreeSet::from([1, 2, 3]));
        assert_eq!(
            reopened.paths.get(&3).map(String::as_str),
            Some("/lib/a.cbz")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fill_adds_ids_and_keeps_existing_paths() {
        let mut s = Snapshot {
            ids: BTreeSet::from([1, 2]),
            paths: BTreeMap::from([(2, "/real.cbz".to_string())]),
        };
        assert!(
            fill_snapshot(&mut s, 1, "/lib/a.cbz"),
            "legacy id gains a path"
        );
        assert!(
            !fill_snapshot(&mut s, 2, "/other.cbz"),
            "existing path kept"
        );
        assert!(fill_snapshot(&mut s, 3, "/lib/c.cbz"), "new id added");
        assert!(!fill_snapshot(&mut s, 3, "/lib/c.cbz"), "idempotent");
        assert_eq!(s.ids, BTreeSet::from([1, 2, 3]));
        assert_eq!(s.paths[&1], "/lib/a.cbz");
        assert_eq!(s.paths[&2], "/real.cbz");
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "br-downloaded-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
