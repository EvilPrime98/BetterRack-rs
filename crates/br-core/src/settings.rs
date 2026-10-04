//! `preferences.sqlite`: `app_settings` (key/value text) and `library_item_prefs`.

use crate::Result;
use crate::db::Db;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_dirs: Vec<String>,
    pub api_url: String,
    pub download_dir: String,
    pub wiki_search: bool,
    pub rescan_on_startup: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_dirs: vec![],
            api_url: String::new(),
            download_dir: String::new(),
            wiki_search: false,
            rescan_on_startup: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPref {
    pub uid: String,
    pub pref_publisher: String,
    pub recursive: bool,
    pub pref_cover: String,
}

pub struct Preferences {
    db: Db,
}

impl Preferences {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Db::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Db::open_in_memory()?)
    }

    fn init(db: Db) -> Result<Self> {
        db.conn().execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT);
             CREATE TABLE IF NOT EXISTS library_item_prefs (
                uid TEXT PRIMARY KEY, pref_publisher TEXT, recursive INTEGER, pref_cover TEXT);",
        )?;
        // The Bun model also seeds from a legacy `.env` / `library-pref.json` on first run. Existing
        // users already migrated through Bun; a fresh Rust-only install has nothing to seed.
        Ok(Self { db })
    }

    pub fn get_app_settings(&self) -> Result<AppSettings> {
        let c = self.db.conn();
        let mut stmt = c.prepare("SELECT key, value FROM app_settings")?;
        let mut values = std::collections::HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))? {
            let (k, v) = row?;
            values.insert(k, v.unwrap_or_default());
        }
        let d = AppSettings::default();
        Ok(AppSettings {
            // JS: `values.outputDirs ? JSON.parse(..) : default` (empty string is falsy).
            output_dirs: match values.get("outputDirs").filter(|v| !v.is_empty()) {
                Some(v) => serde_json::from_str(v)?,
                None => d.output_dirs,
            },
            api_url: values.get("apiUrl").cloned().unwrap_or(d.api_url),
            download_dir: values.get("downloadDir").cloned().unwrap_or(d.download_dir),
            wiki_search: values.get("wikiSearch").map_or(d.wiki_search, |v| v == "true"),
            rescan_on_startup: values.get("rescanOnStartup").map_or(d.rescan_on_startup, |v| v == "true"),
        })
    }

    /// Merge known keys from a JSON object. Unknown keys and nulls are ignored, like Bun.
    pub fn update_app_settings(&self, partial: &Map<String, Value>) -> Result<AppSettings> {
        {
            let c = self.db.conn();
            for (key, value) in partial {
                if !matches!(key.as_str(), "outputDirs" | "apiUrl" | "downloadDir" | "wikiSearch" | "rescanOnStartup") {
                    continue;
                }
                let stored = match value {
                    Value::Null => continue,
                    _ if key == "outputDirs" => value.to_string(),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                c.execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![key, stored],
                )?;
            }
        }
        self.get_app_settings()
    }

    pub fn get_library_pref(&self, uid: &str) -> Result<Option<LibraryPref>> {
        let c = self.db.conn();
        Ok(c.query_row(
            "SELECT uid, pref_publisher, recursive, pref_cover FROM library_item_prefs WHERE uid = ?1",
            [uid],
            row_to_pref,
        )
        .optional()?)
    }

    pub fn get_all_library_prefs(&self) -> Result<Vec<LibraryPref>> {
        let c = self.db.conn();
        let mut stmt = c.prepare("SELECT uid, pref_publisher, recursive, pref_cover FROM library_item_prefs")?;
        let rows = stmt.query_map([], row_to_pref)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn upsert_library_pref(
        &self,
        uid: &str,
        pref_publisher: Option<String>,
        recursive: Option<bool>,
        pref_cover: Option<String>,
    ) -> Result<LibraryPref> {
        let base = self.get_library_pref(uid)?.unwrap_or(LibraryPref {
            uid: uid.to_string(),
            pref_publisher: String::new(),
            recursive: false,
            pref_cover: String::new(),
        });
        let merged = LibraryPref {
            uid: uid.to_string(),
            pref_publisher: pref_publisher.unwrap_or(base.pref_publisher),
            recursive: recursive.unwrap_or(base.recursive),
            pref_cover: pref_cover.unwrap_or(base.pref_cover),
        };
        self.db.conn().execute(
            "INSERT INTO library_item_prefs (uid, pref_publisher, recursive, pref_cover) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid) DO UPDATE SET pref_publisher = excluded.pref_publisher,
                recursive = excluded.recursive, pref_cover = excluded.pref_cover",
            params![merged.uid, merged.pref_publisher, merged.recursive as i64, merged.pref_cover],
        )?;
        Ok(merged)
    }
}

fn row_to_pref(r: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryPref> {
    Ok(LibraryPref {
        uid: r.get(0)?,
        pref_publisher: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        recursive: r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
        pref_cover: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn defaults_then_partial_updates() {
        let p = Preferences::open_in_memory().unwrap();
        assert_eq!(p.get_app_settings().unwrap(), AppSettings::default());
        let s = p
            .update_app_settings(&obj(json!({"apiUrl": "https://x.test/", "wikiSearch": true, "bogus": 1, "downloadDir": null})))
            .unwrap();
        assert_eq!(s.api_url, "https://x.test/");
        assert!(s.wiki_search && !s.rescan_on_startup);
        assert_eq!(s.download_dir, "");
        let s = p.update_app_settings(&obj(json!({"outputDirs": ["C:\\a", "C:\\b"]}))).unwrap();
        assert_eq!(s.output_dirs, vec!["C:\\a", "C:\\b"]);
    }

    #[test]
    fn settings_serialize_as_camel_case() {
        let v = serde_json::to_value(AppSettings::default()).unwrap();
        assert_eq!(v, json!({"outputDirs": [], "apiUrl": "", "downloadDir": "", "wikiSearch": false, "rescanOnStartup": false}));
    }

    #[test]
    fn library_pref_merge() {
        let p = Preferences::open_in_memory().unwrap();
        assert_eq!(p.get_library_pref("u").unwrap(), None);
        let a = p.upsert_library_pref("u", Some("DC".into()), None, None).unwrap();
        assert_eq!((a.pref_publisher.as_str(), a.recursive), ("DC", false));
        let b = p.upsert_library_pref("u", None, Some(true), None).unwrap();
        assert_eq!((b.pref_publisher.as_str(), b.recursive), ("DC", true));
        assert_eq!(p.get_all_library_prefs().unwrap().len(), 1);
    }
}
