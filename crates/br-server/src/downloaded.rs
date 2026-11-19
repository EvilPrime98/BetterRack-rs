//! Post ids whose download was started: a mutex-guarded set persisted whole to `downloaded.json`.

use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE_NAME: &str = "downloaded.json";

pub struct Downloaded {
    file: PathBuf,
    ids: Mutex<BTreeSet<i64>>,
}

impl Downloaded {
    pub fn open(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE_NAME);
        let ids = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            file,
            ids: Mutex::new(ids),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeSet<i64>> {
        self.ids.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<i64> {
        self.lock().iter().copied().collect()
    }

    /// Records `id`; a repeat is a no-op.
    pub fn record(&self, id: i64) {
        let mut ids = self.lock();
        if !ids.insert(id) {
            return;
        }
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let written = serde_json::to_string(&*ids)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&self.file, json).map_err(|e| e.to_string()));
        if let Err(e) = written {
            tracing::warn!(
                "could not save downloaded ids to {}: {e}",
                self.file.display()
            );
        }
    }
}

pub async fn list(State(s): State<AppState>) -> Json<Vec<i64>> {
    Json(s.downloaded.list())
}
