//! `comic-data.sqlite` (`comic_data`): per-uid reading progress, rating and identified metadata.
//!
//! Rows are exposed as JSON objects (`TComicData` in the Bun source) rather than a struct, so the
//! opaque `comic` payload and any extra keys a client sends round-trip untouched. Merge rules
//! follow `ComicDataModel.upsert` exactly: `{uid, ...existing, ...partial}`, with `lastReadAt`
//! bumped only when `currentPage` or `readPer` changes.

use crate::Result;
use crate::db::{Db, add_column_if_missing};
use rusqlite::types::Value as Sql;
use rusqlite::{OptionalExtension, params};
use serde_json::{Map, Number, Value};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ComicRecord = Map<String, Value>;

const SELECT: &str = "SELECT uid, pref_id, source_wiki, meta_source, cover, identified, comic, rating, \
                      current_page, read_per, read, last_read_at FROM comic_data";

pub struct ComicDataStore {
    db: Db,
}

impl ComicDataStore {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Db::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Db::open_in_memory()?)
    }

    fn init(db: Db) -> Result<Self> {
        {
            let c = db.conn();
            c.execute_batch(
                "CREATE TABLE IF NOT EXISTS comic_data (
                    uid TEXT PRIMARY KEY,
                    pref_id INTEGER,
                    source_wiki TEXT,
                    meta_source TEXT,
                    cover TEXT,
                    identified INTEGER,
                    comic TEXT,
                    rating INTEGER,
                    current_page INTEGER,
                    read_per REAL,
                    read INTEGER,
                    last_read_at INTEGER
                )",
            )?;
            for col in ["identified INTEGER", "comic TEXT", "meta_source TEXT", "last_read_at INTEGER"] {
                add_column_if_missing(&c, "comic_data", col);
            }
        }
        Ok(Self { db })
    }

    pub fn get_all(&self) -> Result<ComicRecord> {
        let c = self.db.conn();
        let mut stmt = c.prepare(SELECT)?;
        let mut out = Map::new();
        for row in stmt.query_map([], row_to_record)? {
            let rec = row?;
            let uid = rec["uid"].as_str().unwrap_or_default().to_string();
            out.insert(uid, Value::Object(rec));
        }
        Ok(out)
    }

    pub fn get_by_uid(&self, uid: &str) -> Result<Option<ComicRecord>> {
        let c = self.db.conn();
        Ok(c.query_row(&format!("{SELECT} WHERE uid = ?1"), [uid], row_to_record).optional()?)
    }

    pub fn upsert(&self, uid: &str, partial: &ComicRecord) -> Result<ComicRecord> {
        let existing = self.get_by_uid(uid)?;

        let mut merged = ComicRecord::new();
        merged.insert("uid".into(), uid.into());
        for source in existing.iter().chain(std::iter::once(partial)) {
            for (k, v) in source {
                if k != "uid" {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }

        let changed = ["currentPage", "readPer"].iter().any(|k| {
            !js_strict_eq(merged.get(*k), existing.as_ref().and_then(|e| e.get(*k)))
        });
        let last_read = if changed {
            Some(Value::from(now_ms()))
        } else {
            existing.as_ref().and_then(|e| e.get("lastReadAt")).cloned()
        };
        match last_read {
            Some(v) => merged.insert("lastReadAt".into(), v),
            None => merged.remove("lastReadAt"),
        };

        let comic = match merged.get("comic") {
            Some(v) if truthy(v) => Sql::Text(serde_json::to_string(v)?),
            _ => Sql::Null,
        };
        let params = params![
            uid,
            plain(merged.get("prefId")),
            plain(merged.get("sourceWiki")),
            plain(merged.get("metaSource")),
            comic,
            plain(merged.get("cover")),
            boolean(merged.get("identified")),
            plain(merged.get("rating")),
            plain(merged.get("currentPage")),
            plain(merged.get("readPer")),
            boolean(merged.get("read")),
            plain(merged.get("lastReadAt")),
        ];
        self.db.conn().execute(
            "INSERT INTO comic_data (uid, pref_id, source_wiki, meta_source, comic, cover, identified, rating,
                                     current_page, read_per, read, last_read_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(uid) DO UPDATE SET pref_id = excluded.pref_id, source_wiki = excluded.source_wiki,
                meta_source = excluded.meta_source, comic = excluded.comic, cover = excluded.cover,
                identified = excluded.identified, rating = excluded.rating, current_page = excluded.current_page,
                read_per = excluded.read_per, read = excluded.read, last_read_at = excluded.last_read_at",
            params,
        )?;
        Ok(merged)
    }

    /// `upsert(uid, { identified, comic: undefined, sourceWiki: undefined, metaSource: undefined,
    /// prefId: undefined })`: creates the row if missing, keeps progress and ratings.
    pub fn clear_identification(&self, uid: &str, identified: Option<bool>) -> Result<()> {
        let c = self.db.conn();
        c.execute("INSERT OR IGNORE INTO comic_data (uid) VALUES (?1)", [uid])?;
        c.execute(
            "UPDATE comic_data SET identified = ?2, comic = NULL, source_wiki = NULL, meta_source = NULL, pref_id = NULL WHERE uid = ?1",
            params![uid, identified.map(i64::from)],
        )?;
        Ok(())
    }

    /// Clear identification everywhere (progress and ratings stay).
    pub fn reset_identification(&self) -> Result<()> {
        self.db.conn().execute(
            "UPDATE comic_data SET identified = NULL, comic = NULL, source_wiki = NULL, meta_source = NULL, pref_id = NULL",
            [],
        )?;
        Ok(())
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// JS truthiness for the values that can reach `merged.comic ? ... : null`.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// `a === b` where a missing key is `undefined` and numbers compare by value.
fn js_strict_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `v ?? null` bound to a column: nulls and objects store NULL, booleans store 0/1.
fn plain(v: Option<&Value>) -> Sql {
    match v {
        Some(Value::Bool(b)) => Sql::Integer(*b as i64),
        Some(Value::Number(n)) => n.as_i64().map(Sql::Integer).unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or(0.0))),
        Some(Value::String(s)) => Sql::Text(s.clone()),
        _ => Sql::Null,
    }
}

/// `v === undefined ? null : Number(v)`: an explicit JSON null becomes 0, like JS `Number(null)`.
fn boolean(v: Option<&Value>) -> Sql {
    match v {
        None => Sql::Null,
        Some(Value::Null) => Sql::Integer(0),
        Some(Value::Bool(b)) => Sql::Integer(*b as i64),
        Some(Value::Number(n)) => plain(Some(&Value::Number(n.clone()))),
        _ => Sql::Null,
    }
}

fn num(n: Option<i64>) -> Option<Value> {
    n.map(Value::from)
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<ComicRecord> {
    let mut m = ComicRecord::new();
    m.insert("uid".into(), r.get::<_, String>(0)?.into());
    let mut put = |k: &str, v: Option<Value>| {
        if let Some(v) = v {
            m.insert(k.into(), v);
        }
    };
    put("prefId", num(r.get(1)?));
    put("sourceWiki", r.get::<_, Option<String>>(2)?.map(Value::from));
    put("metaSource", r.get::<_, Option<String>>(3)?.map(Value::from));
    put("identified", r.get::<_, Option<i64>>(5)?.map(|i| Value::Bool(i != 0)));
    // JS: `row.comic ? JSON.parse(row.comic) : undefined`. Unparseable text would throw there; skip it here.
    put(
        "comic",
        r.get::<_, Option<String>>(6)?
            .filter(|s| !s.is_empty())
            .and_then(|s| serde_json::from_str(&s).ok()),
    );
    put("cover", r.get::<_, Option<String>>(4)?.map(Value::from));
    put("rating", num(r.get(7)?));
    put("currentPage", num(r.get(8)?));
    // JS numbers have no int/float split, so whole values print as `1` not `1.0`.
    put(
        "readPer",
        r.get::<_, Option<f64>>(9)?.map(|f| {
            if f.fract() == 0.0 && f.abs() < 9e15 { Value::from(f as i64) } else { Number::from_f64(f).map_or(Value::Null, Value::Number) }
        }),
    );
    put("read", r.get::<_, Option<i64>>(10)?.map(|i| Value::Bool(i != 0)));
    put("lastReadAt", num(r.get(11)?));
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> ComicRecord {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn upsert_merges_and_tracks_last_read() {
        let s = ComicDataStore::open_in_memory().unwrap();
        let a = s.upsert("u1", &obj(json!({"rating": 4}))).unwrap();
        assert_eq!(a["rating"], 4);
        assert!(!a.contains_key("lastReadAt"));

        let b = s.upsert("u1", &obj(json!({"currentPage": 3, "readPer": 0.5}))).unwrap();
        assert_eq!(b["rating"], 4);
        let stamp = b["lastReadAt"].as_i64().unwrap();
        assert!(stamp > 0);

        // Unchanged progress keeps the old timestamp.
        let c = s.upsert("u1", &obj(json!({"rating": 5, "currentPage": 3}))).unwrap();
        assert_eq!(c["lastReadAt"].as_i64().unwrap(), stamp);
        assert_eq!(s.get_by_uid("u1").unwrap().unwrap(), c);
    }

    #[test]
    fn booleans_comic_json_and_whole_floats() {
        let s = ComicDataStore::open_in_memory().unwrap();
        s.upsert(
            "u",
            &obj(json!({"identified": true, "read": false, "readPer": 1.0, "comic": {"title": "X", "z": [1, 2], "a": null}})),
        )
        .unwrap();
        let all = s.get_all().unwrap();
        let row = all["u"].as_object().unwrap();
        assert_eq!(row["identified"], true);
        assert_eq!(row["read"], false);
        assert_eq!(row["readPer"], json!(1));
        assert_eq!(row["comic"], json!({"title": "X", "z": [1, 2], "a": null}));
        assert!(!row.contains_key("rating"));
    }

    #[test]
    fn reset_identification_keeps_progress() {
        let s = ComicDataStore::open_in_memory().unwrap();
        s.upsert("u", &obj(json!({"identified": true, "comic": {"t": 1}, "sourceWiki": "dc", "prefId": 9, "rating": 3}))).unwrap();
        s.reset_identification().unwrap();
        let r = s.get_by_uid("u").unwrap().unwrap();
        assert_eq!(r["rating"], 3);
        assert!(!r.contains_key("identified") && !r.contains_key("comic") && !r.contains_key("prefId"));
    }

    #[test]
    fn opens_a_bun_style_table_missing_newer_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("comic-data.sqlite");
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE comic_data (uid TEXT PRIMARY KEY, pref_id INTEGER, source_wiki TEXT, cover TEXT, rating INTEGER, current_page INTEGER, read_per REAL, read INTEGER);
                            INSERT INTO comic_data (uid, rating, read) VALUES ('old', 2, 1);")
            .unwrap();
        let s = ComicDataStore::open(&path).unwrap();
        let r = s.get_by_uid("old").unwrap().unwrap();
        assert_eq!((r["rating"].as_i64(), r["read"].as_bool()), (Some(2), Some(true)));
        s.upsert("old", &obj(json!({"identified": true, "comic": {"a": 1}}))).unwrap();
    }
}
