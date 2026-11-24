//! A `LibraryFs` over a document-tree provider (Android's Storage Access Framework).
//!
//! A tree is addressed by an opaque document id, not by path. `TreeFs` hands out path-like ids so
//! the rest of the core (joins, `resolve_in`, uids) keeps working: `/saf/<key>/<label>/Series/a.cbz`
//! where `<key>` names a registered tree and `<label>` is its display name. Every other path
//! falls through to `StdFs` (app-private folders).

use crate::fs::{DirEntry, FileMeta, LibraryFs, ReadSeek, StdFs};
use std::collections::HashMap;
use std::io;
use std::sync::{Mutex, RwLock};

pub const PREFIX: &str = "/saf";

#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime_ms: f64,
}

pub trait DocumentProvider: Send + Sync {
    /// Children of the directory document `doc_id` inside the tree `tree`.
    fn children(&self, tree: &str, doc_id: &str) -> io::Result<Vec<Document>>;
    fn open(&self, tree: &str, doc_id: &str) -> io::Result<Box<dyn ReadSeek>>;
}

pub struct TreeFs<P> {
    provider: P,
    /// Tree key -> root document id.
    roots: RwLock<HashMap<String, String>>,
    /// Documents seen by `list_dir`, by virtual path. Lets `stat` and `open` skip a listing.
    seen: Mutex<HashMap<String, Document>>,
}

/// `/saf/<key>/<label>[/rest...]` -> (key, number of segments after the label).
fn split(path: &str) -> Option<(&str, usize)> {
    let rest = path.strip_prefix(PREFIX)?.strip_prefix('/')?;
    let mut it = rest.split('/').filter(|s| !s.is_empty());
    let key = it.next()?;
    it.next()?;
    Some((key, it.count()))
}

/// The virtual path of a tree's root.
pub fn root_path(key: &str, label: &str) -> String {
    format!("{PREFIX}/{key}/{label}")
}

fn not_found(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("not found: {path}"))
}

impl<P: DocumentProvider> TreeFs<P> {
    pub fn new(provider: P) -> Self {
        Self { provider, roots: RwLock::default(), seen: Mutex::default() }
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Makes `/saf/<key>/...` resolvable. Returns the root's virtual path.
    pub fn register(&self, key: &str, label: &str, root_doc_id: &str) -> String {
        self.roots.write().unwrap().insert(key.to_string(), root_doc_id.to_string());
        root_path(key, label)
    }

    pub fn unregister(&self, key: &str) {
        self.roots.write().unwrap().remove(key);
        let prefix = format!("{PREFIX}/{key}/");
        self.seen.lock().unwrap().retain(|p, _| !p.starts_with(&prefix));
    }

    fn root_doc(&self, key: &str) -> io::Result<String> {
        self.roots
            .read()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("unknown tree {key}")))
    }

    /// The document at `path`, or `None` for a tree root.
    fn document(&self, path: &str) -> io::Result<(String, Option<Document>)> {
        let clean = path.trim_end_matches('/');
        let (key, below) = split(clean).ok_or_else(|| not_found(path))?;
        let key = key.to_string();
        self.root_doc(&key)?;
        if below == 0 {
            return Ok((key, None));
        }
        if let Some(d) = self.seen.lock().unwrap().get(clean) {
            return Ok((key, Some(d.clone())));
        }
        let parent = clean.rsplit_once('/').map_or("", |(p, _)| p);
        self.list(parent)?;
        let doc = self.seen.lock().unwrap().get(clean).cloned();
        doc.map(|d| (key, Some(d))).ok_or_else(|| not_found(path))
    }

    fn list(&self, dir: &str) -> io::Result<Vec<DirEntry>> {
        let dir = dir.trim_end_matches('/');
        let (key, doc) = self.document(dir)?;
        let doc_id = match doc {
            Some(d) if !d.is_dir => return Err(io::Error::other(format!("not a directory: {dir}"))),
            Some(d) => d.id,
            None => self.root_doc(&key)?,
        };
        let kids = self.provider.children(&key, &doc_id)?;
        let mut seen = self.seen.lock().unwrap();
        let prefix = format!("{dir}/");
        // Drop stale direct children; deeper entries stay valid until their own listing.
        seen.retain(|p, _| !p.starts_with(&prefix) || p[prefix.len()..].contains('/'));
        Ok(kids
            .into_iter()
            .map(|d| {
                let path = format!("{prefix}{}", d.name);
                let entry = DirEntry { name: d.name.clone(), path: path.clone(), is_dir: d.is_dir };
                seen.insert(path, d);
                entry
            })
            .collect())
    }
}

impl<P: DocumentProvider> LibraryFs for TreeFs<P> {
    fn list_dir(&self, dir: &str) -> io::Result<Vec<DirEntry>> {
        if dir.starts_with(PREFIX) { self.list(dir) } else { StdFs.list_dir(dir) }
    }

    fn stat(&self, path: &str) -> io::Result<FileMeta> {
        if !path.starts_with(PREFIX) {
            return StdFs.stat(path);
        }
        Ok(match self.document(path)?.1 {
            None => FileMeta { is_dir: true, size: 0, mtime_ms: 0.0 },
            Some(d) => FileMeta { is_dir: d.is_dir, size: d.size, mtime_ms: d.mtime_ms },
        })
    }

    fn open(&self, path: &str) -> io::Result<Box<dyn ReadSeek>> {
        if !path.starts_with(PREFIX) {
            return StdFs.open(path);
        }
        match self.document(path)? {
            (key, Some(d)) if !d.is_dir => self.provider.open(&key, &d.id),
            _ => Err(io::Error::other(format!("not a file: {path}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct Fake;
    fn doc(id: &str, name: &str, is_dir: bool, size: u64) -> Document {
        Document { id: id.into(), name: name.into(), is_dir, size, mtime_ms: 5.0 }
    }
    impl DocumentProvider for Fake {
        fn children(&self, tree: &str, id: &str) -> io::Result<Vec<Document>> {
            assert_eq!(tree, "k1");
            Ok(match id {
                "root" => vec![doc("d1", "Series", true, 0), doc("f0", "loose.cbz", false, 9)],
                "d1" => vec![doc("f1", "a.cbz", false, 3)],
                _ => return Err(not_found(id)),
            })
        }
        fn open(&self, _: &str, id: &str) -> io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(Cursor::new(format!("body-{id}").into_bytes())))
        }
    }

    fn fs() -> (TreeFs<Fake>, String) {
        let fs = TreeFs::new(Fake);
        let root = fs.register("k1", "Comics", "root");
        (fs, root)
    }

    #[test]
    fn lists_stats_and_opens_through_virtual_paths() {
        let (fs, root) = fs();
        assert_eq!(root, "/saf/k1/Comics");
        let mut top = fs.list_dir(&root).unwrap();
        top.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(top[1].path, "/saf/k1/Comics/loose.cbz");
        assert!(top[0].is_dir);
        let sub = fs.list_dir(&top[0].path).unwrap();
        assert_eq!(sub[0].path, "/saf/k1/Comics/Series/a.cbz");
        assert_eq!(fs.stat(&sub[0].path).unwrap().size, 3);
        assert_eq!(fs.read(&sub[0].path).unwrap(), b"body-f1");
        assert!(fs.stat(&root).unwrap().is_dir);
    }

    #[test]
    fn stat_and_open_resolve_without_a_prior_listing() {
        let (fs, _) = fs();
        assert_eq!(fs.read("/saf/k1/Comics/Series/a.cbz").unwrap(), b"body-f1");
        assert!(fs.stat("/saf/k1/Comics/Series").unwrap().is_dir);
        assert_eq!(
            fs.stat("/saf/k1/Comics/Series/nope.cbz").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(fs.stat("/saf/other/Comics").is_err());
    }

    #[test]
    fn other_paths_use_the_real_filesystem() {
        let (fs, _) = fs();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x"), b"1").unwrap();
        let root = dir.path().to_string_lossy().into_owned();
        assert_eq!(fs.list_dir(&root).unwrap().len(), 1);
        assert!(fs.stat(&root).unwrap().is_dir);
    }
}
