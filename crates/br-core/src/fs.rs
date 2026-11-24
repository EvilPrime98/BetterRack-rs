//! Read-side access to library files. Paths are opaque strings so an Android implementation can
//! hand out content URIs; the desktop implementation treats them as filesystem paths.

use std::io::{Read, Seek};
use std::path::Path;

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileMeta {
    pub is_dir: bool,
    pub size: u64,
    /// Milliseconds since the epoch, fractional like Node's `mtimeMs`.
    pub mtime_ms: f64,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    /// Opaque id to pass back to `list_dir`, `stat` and `open`.
    pub path: String,
    pub is_dir: bool,
}

pub trait LibraryFs: Send + Sync {
    /// Immediate children. Symlinked folders are reported as non-directories.
    fn list_dir(&self, dir: &str) -> std::io::Result<Vec<DirEntry>>;
    fn stat(&self, path: &str) -> std::io::Result<FileMeta>;
    fn open(&self, path: &str) -> std::io::Result<Box<dyn ReadSeek>>;

    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.open(path)?.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdFs;

impl LibraryFs for StdFs {
    fn list_dir(&self, dir: &str) -> std::io::Result<Vec<DirEntry>> {
        Ok(std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| DirEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path().to_string_lossy().into_owned(),
                is_dir: e.file_type().is_ok_and(|t| t.is_dir()),
            })
            .collect())
    }

    fn stat(&self, path: &str) -> std::io::Result<FileMeta> {
        let meta = std::fs::metadata(path)?;
        let since = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .unwrap_or_default();
        Ok(FileMeta {
            is_dir: meta.is_dir(),
            size: meta.len(),
            mtime_ms: since.as_secs() as f64 * 1e3 + f64::from(since.subsec_nanos()) / 1e6,
        })
    }

    fn open(&self, path: &str) -> std::io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(std::fs::File::open(Path::new(path))?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_fs_lists_stats_and_reads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("a.cbz"), b"abc").unwrap();
        let root = dir.path().to_string_lossy().into_owned();

        let mut kids = StdFs.list_dir(&root).unwrap();
        kids.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(kids.len(), 2);
        assert!(!kids[0].is_dir && kids[1].is_dir);

        let meta = StdFs.stat(&kids[0].path).unwrap();
        assert_eq!((meta.is_dir, meta.size), (false, 3));
        assert!(StdFs.stat(&kids[1].path).unwrap().is_dir);
        assert_eq!(StdFs.read(&kids[0].path).unwrap(), b"abc");
        assert!(StdFs.stat(&format!("{root}/missing")).is_err());
    }
}
