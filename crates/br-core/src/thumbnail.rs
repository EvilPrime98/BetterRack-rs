//! Cover thumbnails: first page of the archive, resized to
//! `THUMBNAIL_WIDTH` (never enlarged), lossy WebP at `THUMBNAIL_QUALITY`, cached on disk as
//! `<cache>/<uid>/<uid>.webp` (an existing cache is reused). A failed file is not retried until its size/mtime changes.
//!
//! There is no raw-extract directory: the first
//! page is read from the archive into memory and encoded in-process.

use crate::archive::{Archives, file_stamp};
use crate::sync::{Semaphore, SingleFlight};
use crate::{CoreError, Result};
use image::imageops::FilterType;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub const THUMBNAIL_WIDTH: u32 = 180;
pub const THUMBNAIL_QUALITY: f32 = 82.0;
pub const EXTRACT_CONCURRENCY: usize = 4;
pub const CACHE_DIR_NAME: &str = "tmp-thumbnails";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The uid becomes a directory name, so only the characters of a real uid are accepted.
fn is_safe_uid(uid: &str) -> bool {
    !uid.is_empty() && uid.len() <= 64 && uid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Decode, resize to `width` (keeping the aspect ratio, never enlarging) and encode lossy WebP.
pub fn encode_thumbnail(source: &[u8], width: u32, quality: f32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(source)
        .map_err(|e| CoreError::Invalid(format!("cannot decode image: {e}")))?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(CoreError::Invalid("empty image".into()));
    }
    let (tw, th) = if w > width {
        (
            width,
            ((f64::from(h) * f64::from(width) / f64::from(w)).round() as u32).max(1),
        )
    } else {
        (w, h)
    };
    let encoded = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        let out = if (tw, th) == (w, h) {
            rgba
        } else {
            image::imageops::resize(&rgba, tw, th, FilterType::Lanczos3)
        };
        webp::Encoder::from_rgba(out.as_raw(), tw, th)
            .encode(quality)
            .to_vec()
    } else {
        let rgb = img.to_rgb8();
        let out = if (tw, th) == (w, h) {
            rgb
        } else {
            image::imageops::resize(&rgb, tw, th, FilterType::Lanczos3)
        };
        webp::Encoder::from_rgb(out.as_raw(), tw, th)
            .encode(quality)
            .to_vec()
    };
    Ok(encoded)
}

pub struct Thumbnails {
    cache_dir: PathBuf,
    archives: Arc<Archives>,
    resolved: Mutex<HashMap<String, PathBuf>>,
    failed: Mutex<HashMap<String, String>>,
    flight: SingleFlight<String, Option<PathBuf>>,
    limiter: Semaphore,
}

impl Thumbnails {
    pub fn new(cache_dir: PathBuf, archives: Arc<Archives>) -> Self {
        Self {
            cache_dir,
            archives,
            resolved: Mutex::default(),
            failed: Mutex::default(),
            flight: SingleFlight::default(),
            limiter: Semaphore::new(EXTRACT_CONCURRENCY),
        }
    }

    fn uid_dir(&self, uid: &str) -> PathBuf {
        self.cache_dir.join(uid)
    }

    fn signature(path: &Path) -> Option<String> {
        file_stamp(path)
            .ok()
            .map(|s| format!("{}:{}", s.size, s.mtime_ms))
    }

    fn generate(&self, uid: &str, file: &Path) -> Result<Option<PathBuf>> {
        let _permit = self.limiter.acquire();
        let pages = self.archives.list_pages(file)?;
        let Some(first) = pages.first() else {
            return Ok(None);
        };
        let raw = self.archives.read_entry(file, first)?;
        let webp = encode_thumbnail(&raw, THUMBNAIL_WIDTH, THUMBNAIL_QUALITY)?;

        let dir = self.uid_dir(uid);
        std::fs::create_dir_all(&dir)?;
        let out = dir.join(format!("{uid}.webp"));
        // Written aside and renamed so a concurrent cache lookup never sees half a file.
        let part = dir.join(format!("{uid}.webp.part"));
        std::fs::write(&part, &webp)?;
        std::fs::rename(&part, &out)?;
        lock(&self.resolved).insert(uid.to_string(), out.clone());
        Ok(Some(out))
    }

    fn attempt(&self, uid: &str, file: &Path) -> Option<PathBuf> {
        let signature = Self::signature(file)?;
        if lock(&self.failed).get(uid) == Some(&signature) {
            return None;
        }
        let generated = self.generate(uid, file).unwrap_or_else(|e| {
            tracing::error!(err = %e, uid, "failed to generate thumbnail");
            None
        });
        if generated.is_some() {
            lock(&self.failed).remove(uid);
        } else {
            lock(&self.failed).insert(uid.to_string(), signature);
        }
        generated
    }

    fn find_cached(&self, uid: &str) -> Option<PathBuf> {
        let dir = self.uid_dir(uid);
        let preferred = dir.join(format!("{uid}.webp"));
        let found = if preferred.is_file() {
            Some(preferred)
        } else {
            std::fs::read_dir(&dir)
                .ok()?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.is_file() && p.extension().is_none_or(|x| x != "part"))
        }?;
        lock(&self.resolved).insert(uid.to_string(), found.clone());
        Some(found)
    }

    /// Cached thumbnail path, generating it from `file` when missing. Blocking.
    pub fn get_thumbnail(&self, uid: &str, file: Option<&Path>) -> Option<PathBuf> {
        if !is_safe_uid(uid) {
            return None;
        }
        let memoised = lock(&self.resolved).get(uid).cloned();
        if let Some(path) = memoised {
            if path.is_file() {
                return Some(path);
            }
            lock(&self.resolved).remove(uid);
        }
        if let Some(cached) = self.find_cached(uid) {
            return Some(cached);
        }
        let file = file?;
        self.flight
            .run(&uid.to_string(), || self.attempt(uid, file))
    }

    /// Forget everything about `uid` and regenerate. Blocking.
    pub fn retry(&self, uid: &str, file: &Path) -> Option<PathBuf> {
        if !is_safe_uid(uid) {
            return None;
        }
        self.flight.wait_idle(&uid.to_string());
        lock(&self.failed).remove(uid);
        lock(&self.resolved).remove(uid);
        let _ = std::fs::remove_dir_all(self.uid_dir(uid));
        self.get_thumbnail(uid, Some(file))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn webp_size(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP).unwrap();
        (img.width(), img.height())
    }

    fn make_cbz(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn resizes_to_width_keeping_aspect_and_never_enlarges() {
        assert_eq!(
            webp_size(&encode_thumbnail(&png(400, 600), 180, 82.0).unwrap()),
            (180, 270)
        );
        assert_eq!(
            webp_size(&encode_thumbnail(&png(100, 150), 180, 82.0).unwrap()),
            (100, 150)
        );
        assert!(encode_thumbnail(b"not an image", 180, 82.0).is_err());
    }

    #[test]
    fn generates_caches_and_retries() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = dir.path().join("a.cbz");
        make_cbz(&cbz, &[("2.png", png(10, 10)), ("1.png", png(360, 360))]);
        let thumbs = Thumbnails::new(dir.path().join("cache"), Arc::new(Archives::new(None)));

        let uid = "0123abcd-0000-0000-0000-000000000000";
        let path = thumbs.get_thumbnail(uid, Some(&cbz)).expect("generated");
        assert_eq!(
            path,
            dir.path()
                .join("cache")
                .join(uid)
                .join(format!("{uid}.webp"))
        );
        assert_eq!(
            webp_size(&std::fs::read(&path).unwrap()),
            (180, 180),
            "first page in natural order is used"
        );

        // Served from the cache without the archive.
        std::fs::remove_file(&cbz).unwrap();
        assert_eq!(thumbs.get_thumbnail(uid, None), Some(path.clone()));
        // A fresh instance finds the file on disk.
        let again = Thumbnails::new(dir.path().join("cache"), Arc::new(Archives::new(None)));
        assert_eq!(again.get_thumbnail(uid, None), Some(path.clone()));

        // Retry drops the cache and regenerates from the file.
        make_cbz(&cbz, &[("1.png", png(200, 100))]);
        let regenerated = again.retry(uid, &cbz).expect("regenerated");
        assert_eq!(webp_size(&std::fs::read(regenerated).unwrap()), (180, 90));
    }

    #[test]
    fn broken_files_are_remembered_until_they_change() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = dir.path().join("bad.cbz");
        make_cbz(&cbz, &[("1.png", b"garbage".to_vec())]);
        let thumbs = Thumbnails::new(dir.path().join("cache"), Arc::new(Archives::new(None)));
        let uid = "bad-uid";

        assert_eq!(thumbs.get_thumbnail(uid, Some(&cbz)), None);
        // Fixing the content at the same size and mtime would still be skipped; a change is not.
        make_cbz(&cbz, &[("1.png", png(50, 50)), ("2.png", png(50, 50))]);
        assert!(thumbs.get_thumbnail(uid, Some(&cbz)).is_some());
    }

    #[test]
    fn unsafe_uids_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbnails::new(dir.path().to_path_buf(), Arc::new(Archives::new(None)));
        assert_eq!(thumbs.get_thumbnail("..", None), None);
        assert_eq!(thumbs.get_thumbnail("a/b", None), None);
        assert_eq!(thumbs.get_thumbnail("", None), None);
    }
}
