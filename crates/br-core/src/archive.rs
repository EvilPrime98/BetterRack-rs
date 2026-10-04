//! Archive layer: entry lists, page lists, entry bytes and `ComicInfo.xml`.
//!
//! `.cbz`/`.zip` are read in-process with the `zip` crate; everything else (`.cbr`, `.cb7`,
//! `.cbt`, `.rar`, `.7z`) goes through the bundled 7-Zip. A zip the crate cannot read
//! (odd compression, a misnamed RAR) silently falls back to 7-Zip too.
//!
//! All methods block (file IO, child processes); call them from `spawn_blocking`.

use crate::collate::locale_compare_numeric;
use crate::comic_info::{self, Bookmark, ComicInfo};
use crate::{CoreError, Result};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

pub const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const ARCHIVE_CACHE_MAX_SIZE: usize = 50;

pub const SEVEN_ZIP_ENV_VAR: &str = "SEVEN_ZIP_PATH";
#[cfg(windows)]
const BIN_NAMES: &[&str] = &["7z"];
#[cfg(not(windows))]
const BIN_NAMES: &[&str] = &["7zz", "7z", "7za"];
#[cfg(windows)]
const FALLBACK_PATHS: &[&str] = &[
    r"C:\Program Files\7-Zip\7z.exe",
    r"C:\Program Files (x86)\7-Zip\7z.exe",
];
#[cfg(not(windows))]
const FALLBACK_PATHS: &[&str] = &[
    "/usr/bin/7zz",
    "/usr/local/bin/7zz",
    "/usr/bin/7z",
    "/usr/local/bin/7z",
    "/usr/bin/7za",
    "/usr/local/bin/7za",
];

fn invalid(msg: impl Into<String>) -> CoreError {
    CoreError::Invalid(msg.into())
}

/// Lowercased extension without the dot, like `path.extname(..).toLowerCase()` minus the dot.
pub fn extension(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.rfind('.') {
        Some(i) if i > 0 => base[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

pub fn is_image(entry: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&extension(entry).as_str())
}

pub fn page_mime_type(entry: &str) -> &'static str {
    match extension(entry).as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

/// Reject entry names that could escape the extraction folder (`..`, absolute, drive, empty parts).
pub fn is_safe_entry_name(name: &str) -> bool {
    if name.trim().is_empty() || name.starts_with('/') || name.starts_with('\\') {
        return false;
    }
    let b = name.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return false;
    }
    !name
        .replace('\\', "/")
        .split('/')
        .any(|s| s.is_empty() || s == "..")
}

/// Size and mtime (ms, fractional like Node's `mtimeMs`) of a file: the identity of its bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_ms: f64,
}

pub fn file_stamp(path: &Path) -> std::io::Result<FileStamp> {
    let meta = std::fs::metadata(path)?;
    let since = meta
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(FileStamp {
        size: meta.len(),
        mtime_ms: since.as_secs() as f64 * 1e3 + f64::from(since.subsec_nanos()) / 1e6,
    })
}

/// Small LRU keyed by archive path; an entry is valid only for the stamp it was stored with, so
/// replacing the file on disk invalidates it.
struct Cache<V> {
    map: HashMap<String, (FileStamp, V)>,
    order: VecDeque<String>,
}

impl<V: Clone> Cache<V> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn touch(&mut self, key: &str) {
        self.order.retain(|k| k != key);
        self.order.push_back(key.to_string());
    }

    fn get(&mut self, key: &str, stamp: FileStamp) -> Option<V> {
        match self.map.get(key) {
            Some((s, v)) if *s == stamp => {
                let v = v.clone();
                self.touch(key);
                Some(v)
            }
            Some(_) => {
                self.remove(key);
                None
            }
            None => None,
        }
    }

    fn put(&mut self, key: &str, stamp: FileStamp, value: V) {
        if !self.map.contains_key(key)
            && self.map.len() >= ARCHIVE_CACHE_MAX_SIZE
            && let Some(oldest) = self.order.pop_front()
        {
            self.map.remove(&oldest);
        }
        self.map.insert(key.to_string(), (stamp, value));
        self.touch(key);
    }

    fn remove(&mut self, key: &str) {
        self.map.remove(key);
        self.order.retain(|k| k != key);
    }
}

pub struct Archives {
    configured_7z: Option<PathBuf>,
    resolved_7z: OnceLock<std::result::Result<PathBuf, String>>,
    entries: Mutex<Cache<Vec<String>>>,
    infos: Mutex<Cache<Option<ComicInfo>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(windows)]
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    std::env::split_paths(&path).find_map(|dir| {
        exts.split(';')
            .filter(|e| !e.is_empty())
            .map(|e| dir.join(format!("{name}{}", e.to_lowercase())))
            .find(|p| p.is_file())
    })
}

#[cfg(not(windows))]
fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Locate 7-Zip: configured path, then PATH, then the usual install folders.
pub fn resolve_seven_zip(configured: Option<&Path>) -> std::result::Result<PathBuf, String> {
    if let Some(p) = configured.filter(|p| p.exists()) {
        return Ok(p.to_path_buf());
    }
    if let Some(found) = BIN_NAMES.iter().find_map(|n| which(n)) {
        return Ok(found);
    }
    if let Some(found) = FALLBACK_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    {
        return Ok(found);
    }
    let hint = configured
        .map(|p| {
            format!(
                " {SEVEN_ZIP_ENV_VAR} points to \"{}\", which does not exist.",
                p.display()
            )
        })
        .unwrap_or_default();
    Err(format!(
        "7z executable not found.{hint} Set {SEVEN_ZIP_ENV_VAR} to a 7-Zip binary, or install 7-Zip (7zz/7z/7za) and add it to PATH."
    ))
}

fn is_zip_file(path: &Path) -> bool {
    matches!(extension(&path.to_string_lossy()).as_str(), "cbz" | "zip")
}

/// 7-Zip prints entry paths with the platform separator; mirror that for zip-crate listings so
/// both backends return the same names (they are echoed in `/read/:uid`).
fn native_separators(name: &str) -> String {
    if cfg!(windows) {
        name.replace('/', "\\")
    } else {
        name.to_string()
    }
}

impl Archives {
    pub fn new(configured_7z: Option<PathBuf>) -> Self {
        Self {
            configured_7z,
            resolved_7z: OnceLock::new(),
            entries: Mutex::new(Cache::new()),
            infos: Mutex::new(Cache::new()),
        }
    }

    fn seven_zip(&self) -> Result<&Path> {
        self.resolved_7z
            .get_or_init(|| resolve_seven_zip(self.configured_7z.as_deref()))
            .as_ref()
            .map(PathBuf::as_path)
            .map_err(|e| invalid(e.clone()))
    }

    fn command(&self) -> Result<Command> {
        let mut cmd = Command::new(self.seven_zip()?);
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // CREATE_NO_WINDOW
        cmd.stdin(Stdio::null());
        Ok(cmd)
    }

    pub fn evict(&self, path: &Path) {
        let key = path.to_string_lossy();
        lock(&self.entries).remove(&key);
        lock(&self.infos).remove(&key);
    }

    /// Every non-directory entry, in archive order.
    pub fn list_entries(&self, path: &Path) -> Result<Vec<String>> {
        let key = path.to_string_lossy().into_owned();
        let stamp = file_stamp(path)?;
        if let Some(hit) = lock(&self.entries).get(&key, stamp) {
            return Ok(hit);
        }
        let entries = match is_zip_file(path).then(|| list_zip(path)).flatten() {
            Some(e) => e,
            None => self.list_7z(path)?,
        };
        lock(&self.entries).put(&key, stamp, entries.clone());
        Ok(entries)
    }

    fn list_7z(&self, path: &Path) -> Result<Vec<String>> {
        let out = self
            .command()?
            .args(["l", "-slt", "-ba"])
            .arg(path)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let err = err.trim();
            return Err(invalid(format!(
                "Listing archive failed with code {}{}",
                out.status.code().unwrap_or(-1),
                if err.is_empty() {
                    String::new()
                } else {
                    format!(": {err}")
                }
            )));
        }
        Ok(parse_7z_listing(&String::from_utf8_lossy(&out.stdout)))
    }

    /// Image entries in natural order: the page order that saved progress refers to.
    pub fn list_pages(&self, path: &Path) -> Result<Vec<String>> {
        let mut pages: Vec<String> = self
            .list_entries(path)?
            .into_iter()
            .filter(|e| is_image(e))
            .collect();
        pages.sort_by(|a, b| locale_compare_numeric(a, b));
        if pages.is_empty() {
            return Err(invalid(format!(
                "No image pages found in \"{}\".",
                path.display()
            )));
        }
        Ok(pages)
    }

    /// Bytes of one entry. Errors if the entry is missing or empty.
    pub fn read_entry(&self, path: &Path, entry: &str) -> Result<Vec<u8>> {
        if !is_safe_entry_name(entry) {
            return Err(invalid(format!(
                "Rejected suspicious archive entry path: \"{entry}\""
            )));
        }
        if !path.exists() {
            return Err(invalid(format!(
                "Archive not found: \"{}\"",
                path.display()
            )));
        }
        if is_zip_file(path)
            && let Some(bytes) = read_zip_entry(path, entry)
            && !bytes.is_empty()
        {
            return Ok(bytes);
        }
        self.read_entry_7z(path, entry)
    }

    fn read_entry_7z(&self, path: &Path, entry: &str) -> Result<Vec<u8>> {
        let out = self
            .command()?
            .arg("x")
            .arg(path)
            .arg(entry)
            .args(["-so", "-y"])
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()?;
        let shown = path.display();
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let err = err.trim();
            return Err(invalid(format!(
                "Failed to read page \"{entry}\" from \"{shown}\": extraction process exited with code {}{}",
                out.status.code().unwrap_or(-1),
                if err.is_empty() {
                    String::new()
                } else {
                    format!(": {err}")
                }
            )));
        }
        if out.stdout.is_empty() {
            return Err(invalid(format!(
                "Failed to read page \"{entry}\" from \"{shown}\": no data was produced; the entry may not exist in the archive."
            )));
        }
        Ok(out.stdout)
    }

    /// Extract the given entries into `out_dir` with 7-Zip (`extractEntries`), keeping their
    /// folder structure.
    pub fn extract_entries(&self, path: &Path, out_dir: &Path, entries: &[String]) -> Result<()> {
        if let Some(bad) = entries.iter().find(|e| !is_safe_entry_name(e)) {
            return Err(invalid(format!(
                "Rejected suspicious archive entry path: \"{bad}\""
            )));
        }
        if entries.is_empty() {
            return Ok(());
        }
        let mut out_arg = std::ffi::OsString::from("-o");
        out_arg.push(out_dir);
        let out = self
            .command()?
            .arg("x")
            .arg(path)
            .arg(out_arg)
            .args(entries)
            .arg("-y")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let err = err.trim();
            return Err(invalid(format!(
                "Extraction failed with code {}{}",
                out.status.code().unwrap_or(-1),
                if err.is_empty() {
                    String::new()
                } else {
                    format!(": {err}")
                }
            )));
        }
        Ok(())
    }

    fn sidecar_comic_info(path: &Path) -> Option<String> {
        let stem = path.file_stem()?;
        let sidecar = path.with_file_name(format!("{}.xml", stem.to_string_lossy()));
        std::fs::read(sidecar)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    /// The embedded `ComicInfo.xml` (top level only), else the `<name>.xml` next to the archive.
    pub fn comic_info(&self, path: &Path) -> Result<Option<ComicInfo>> {
        let key = path.to_string_lossy().into_owned();
        let stamp = file_stamp(path)?;
        if let Some(hit) = lock(&self.infos).get(&key, stamp) {
            return Ok(hit);
        }
        let entry = self.list_entries(path)?.into_iter().find(|e| {
            let n = e.replace('\\', "/");
            !n.contains('/') && n.eq_ignore_ascii_case("comicinfo.xml")
        });
        let xml = match entry {
            Some(e) => Some(String::from_utf8_lossy(&self.read_entry(path, &e)?).into_owned()),
            None => Self::sidecar_comic_info(path),
        };
        let info = xml.and_then(|x| comic_info::parse(&x));
        lock(&self.infos).put(&key, stamp, info.clone());
        Ok(info)
    }

    pub fn bookmarks(&self, path: &Path) -> Result<Vec<Bookmark>> {
        Ok(self
            .comic_info(path)?
            .map(|i| i.bookmarks())
            .unwrap_or_default())
    }
}

fn list_zip(path: &Path) -> Option<Vec<String>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).ok()?;
        if !f.is_dir() {
            out.push(native_separators(f.name()));
        }
    }
    Some(out)
}

fn read_zip_entry(path: &Path, entry: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let slashed = entry.replace('\\', "/");
    let index = zip
        .index_for_name(&slashed)
        .or_else(|| zip.index_for_name(entry))?;
    let mut file = zip.by_index(index).ok()?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// `7z l -slt -ba` output: blank-line separated `Key = Value` blocks.
fn parse_7z_listing(output: &str) -> Vec<String> {
    output
        .replace("\r\n", "\n")
        .split("\n\n")
        .filter_map(|block| {
            let field = |key: &str| block.lines().find_map(|l| l.strip_prefix(key));
            let name = field("Path = ").filter(|p| !p.is_empty())?;
            let is_dir = field("Attributes = ").is_some_and(|a| a.contains('D'));
            (!is_dir).then(|| name.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_cbz(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (n, data) in entries {
            zip.start_file(*n, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn extension_and_mime() {
        assert_eq!(extension("a/b/Page.JPG"), "jpg");
        assert_eq!(extension(r"a\b\.hidden"), "");
        assert!(is_image("x.WebP") && !is_image("ComicInfo.xml"));
        assert_eq!(page_mime_type("x.jpeg"), "image/jpeg");
        assert_eq!(page_mime_type("x.bin"), "application/octet-stream");
    }

    #[test]
    fn entry_name_safety() {
        for bad in [
            "",
            " ",
            "../x.png",
            "a/../x.png",
            "/abs.png",
            r"\abs.png",
            "C:x.png",
            "a//b.png",
            "a/",
        ] {
            assert!(!is_safe_entry_name(bad), "{bad:?}");
        }
        for good in ["x.png", "dir/x.png", r"dir\x.png", "..x.png"] {
            assert!(is_safe_entry_name(good), "{good:?}");
        }
    }

    #[test]
    fn parses_7z_listing_skipping_directories() {
        let out = "Path = a\r\nSize = 0\r\nAttributes = D\r\n\r\nPath = a\\1.png\r\nSize = 3\r\nAttributes = A\r\n\r\nPath = b.jpg\r\nAttributes = A\r\n";
        assert_eq!(parse_7z_listing(out), vec!["a\\1.png", "b.jpg"]);
    }

    #[test]
    fn zip_pages_are_in_natural_order_and_non_images_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_cbz(
            dir.path(),
            "t.cbz",
            &[
                ("page10.png", b"10"),
                ("page2.png", b"2"),
                ("page1.png", b"1"),
                ("ComicInfo.xml", b"<ComicInfo/>"),
                ("notes.txt", b"x"),
            ],
        );
        let a = Archives::new(None);
        assert_eq!(
            a.list_pages(&p).unwrap(),
            ["page1.png", "page2.png", "page10.png"]
        );
        assert_eq!(a.read_entry(&p, "page2.png").unwrap(), b"2");
    }

    #[test]
    fn zip_without_images_errors_like_bun() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_cbz(dir.path(), "t.cbz", &[("a.txt", b"x")]);
        let err = Archives::new(None).list_pages(&p).unwrap_err().to_string();
        assert!(err.starts_with("No image pages found in"), "{err}");
    }

    #[test]
    fn unsafe_and_missing_entries_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_cbz(dir.path(), "t.cbz", &[("a.png", b"x")]);
        let a = Archives::new(None);
        assert!(
            a.read_entry(&p, "../a.png")
                .unwrap_err()
                .to_string()
                .starts_with("Rejected suspicious")
        );
        assert!(
            a.read_entry(&dir.path().join("none.cbz"), "a.png")
                .unwrap_err()
                .to_string()
                .starts_with("Archive not found")
        );
    }

    #[test]
    fn comic_info_from_archive_bookmarks_and_cache_invalidation() {
        let dir = tempfile::tempdir().unwrap();
        let xml = br#"<ComicInfo><Series>S</Series><Pages><Page Image="1" Bookmark="Two"/></Pages></ComicInfo>"#;
        let p = make_cbz(
            dir.path(),
            "t.cbz",
            &[("1.png", b"a"), ("2.png", b"b"), ("ComicInfo.xml", xml)],
        );
        let a = Archives::new(None);
        assert_eq!(a.comic_info(&p).unwrap().unwrap().text("Series"), Some("S"));
        assert_eq!(
            a.bookmarks(&p).unwrap(),
            vec![Bookmark {
                page: 2,
                label: "Two".into()
            }]
        );
        // Replace the file: the stamp changes, so the cached list must not be served.
        make_cbz(
            dir.path(),
            "t.cbz",
            &[("1.png", b"a"), ("2.png", b"b"), ("3.png", b"c")],
        );
        assert_eq!(a.list_pages(&p).unwrap().len(), 3);
        assert!(a.comic_info(&p).unwrap().is_none());
    }

    #[test]
    fn nested_comic_info_is_ignored_and_sidecar_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let p = make_cbz(
            dir.path(),
            "t.cbz",
            &[
                ("1.png", b"a"),
                (
                    "sub/ComicInfo.xml",
                    b"<ComicInfo><Series>Nested</Series></ComicInfo>",
                ),
            ],
        );
        let a = Archives::new(None);
        assert!(a.comic_info(&p).unwrap().is_none());
        a.evict(&p);
        std::fs::write(
            dir.path().join("t.xml"),
            "<ComicInfo><Series>Side</Series></ComicInfo>",
        )
        .unwrap();
        assert_eq!(
            a.comic_info(&p).unwrap().unwrap().text("Series"),
            Some("Side")
        );
    }

    #[test]
    fn lru_evicts_the_oldest_beyond_capacity() {
        let mut c: Cache<u32> = Cache::new();
        let s = FileStamp {
            size: 1,
            mtime_ms: 1.0,
        };
        for i in 0..=ARCHIVE_CACHE_MAX_SIZE {
            c.put(&i.to_string(), s, i as u32);
        }
        assert!(c.get("0", s).is_none());
        assert_eq!(c.get("1", s), Some(1));
    }
}
