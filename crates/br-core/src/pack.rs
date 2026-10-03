//! `PackExtractor`: a download that turns out to be a wrapper around several comics (a "weekly
//! pack") is unpacked into a folder next to it; a `.zip`/`.rar` of loose page images is renamed
//! to `.cbz`. Blocking (file system + 7-Zip); call from `spawn_blocking`.

use crate::Result;
use crate::archive::{Archives, extension};
use std::path::{Path, PathBuf};

const COMIC_MEMBER_EXTENSIONS: [&str; 4] = ["cbz", "cbr", "cb7", "cbt"];
const IMAGE_MEMBER_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const NESTED_ARCHIVE_EXTENSIONS: [&str; 3] = ["zip", "rar", "7z"];
const INSPECTABLE_EXTENSIONS: [&str; 5] = ["zip", "rar", "7z", "cbz", "cbr"];
/// Below this size an archive is not opened to look for multiple comics.
pub const SIZE_GATE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_NESTED_DEPTH: u32 = 2;

/// The two archive operations the extractor needs (a fake in tests).
pub trait PackDecompressor: Send + Sync {
    fn list_entries(&self, file: &Path) -> Result<Vec<String>>;
    fn extract_entries(&self, file: &Path, out_dir: &Path, entries: &[String]) -> Result<()>;
}

impl PackDecompressor for Archives {
    fn list_entries(&self, file: &Path) -> Result<Vec<String>> {
        Archives::list_entries(self, file)
    }

    fn extract_entries(&self, file: &Path, out_dir: &Path, entries: &[String]) -> Result<()> {
        Archives::extract_entries(self, file, out_dir, entries)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackAction {
    Extracted,
    Renamed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct PackResult {
    pub action: PackAction,
    pub members: Vec<PathBuf>,
    pub wrapper_removed: bool,
    pub dest_dir: Option<PathBuf>,
    pub renamed_to: Option<PathBuf>,
}

impl PackResult {
    fn skipped() -> Self {
        Self { action: PackAction::Skipped, members: vec![], wrapper_removed: false, dest_dir: None, renamed_to: None }
    }
}

pub struct PackExtractor {
    decompressor: std::sync::Arc<dyn PackDecompressor>,
}

fn dedupe(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }
    let dir = target.parent().unwrap_or(Path::new("."));
    let ext = target.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let stem = target.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut index = 2;
    loop {
        let candidate = dir.join(format!("{stem} ({index}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
        index += 1;
    }
}

fn collect_files(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, extensions, out)?;
        } else if entry.file_type()?.is_file() && extensions.contains(&extension(&entry.file_name().to_string_lossy()).as_str()) {
            out.push(path);
        }
    }
    Ok(())
}

impl PackExtractor {
    pub fn new(decompressor: std::sync::Arc<dyn PackDecompressor>) -> Self {
        Self { decompressor }
    }

    pub fn should_inspect(&self, file: &Path, size_bytes: u64) -> bool {
        INSPECTABLE_EXTENSIONS.contains(&extension(&file.to_string_lossy()).as_str()) && size_bytes >= SIZE_GATE_BYTES
    }

    pub fn extract_pack(&self, file: &Path, output_dir: &Path, on_progress: Option<&dyn Fn(usize, usize)>) -> Result<PackResult> {
        self.extract_pack_at(file, output_dir, on_progress, 0)
    }

    fn extract_pack_at(&self, file: &Path, output_dir: &Path, on_progress: Option<&dyn Fn(usize, usize)>, depth: u32) -> Result<PackResult> {
        let Ok(entries) = self.decompressor.list_entries(file) else { return Ok(PackResult::skipped()) };

        let (mut comics, mut images, mut archives) = (vec![], vec![], vec![]);
        for entry in entries {
            let ext = extension(&entry);
            if COMIC_MEMBER_EXTENSIONS.contains(&ext.as_str()) {
                comics.push(entry);
            } else if IMAGE_MEMBER_EXTENSIONS.contains(&ext.as_str()) {
                images.push(entry);
            } else if NESTED_ARCHIVE_EXTENSIONS.contains(&ext.as_str()) {
                archives.push(entry);
            }
        }
        let ext = extension(&file.to_string_lossy());

        if comics.is_empty() && archives.is_empty() {
            if images.is_empty() || ext == "cbz" || ext == "cbr" {
                return Ok(PackResult::skipped());
            }
            let renamed_to = dedupe(&file.with_extension("cbz"));
            std::fs::rename(file, &renamed_to)?;
            return Ok(PackResult { action: PackAction::Renamed, members: vec![renamed_to.clone()], wrapper_removed: true, dest_dir: None, renamed_to: Some(renamed_to) });
        }

        let can_recurse = depth < MAX_NESTED_DEPTH;
        let to_extract: Vec<String> = if can_recurse { comics.iter().chain(&archives).cloned().collect() } else { comics };
        if to_extract.is_empty() {
            return Ok(PackResult::skipped());
        }

        std::fs::create_dir_all(output_dir)?;
        let temp_dir = output_dir.join(format!(".betterrack-pack-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]));
        std::fs::create_dir(&temp_dir)?;

        let run = || -> Result<PackResult> {
            let total = to_extract.len();
            if let Some(p) = on_progress {
                p(0, total);
            }
            for (i, entry) in to_extract.iter().enumerate() {
                self.decompressor.extract_entries(file, &temp_dir, std::slice::from_ref(entry))?;
                if let Some(p) = on_progress {
                    p(i + 1, total);
                }
            }

            if can_recurse && !archives.is_empty() {
                let mut nested = vec![];
                collect_files(&temp_dir, &NESTED_ARCHIVE_EXTENSIONS, &mut nested)?;
                for nested_archive in nested {
                    let inner = self.extract_pack_at(&nested_archive, &temp_dir, None, depth + 1)?;
                    if inner.action != PackAction::Skipped {
                        let _ = std::fs::remove_file(&nested_archive);
                    }
                }
            }

            let pack_name = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let dest_dir = dedupe(&output_dir.join(pack_name));
            std::fs::rename(&temp_dir, &dest_dir)?;

            let mut members = vec![];
            collect_files(&dest_dir, &COMIC_MEMBER_EXTENSIONS, &mut members)?;
            let _ = std::fs::remove_file(file);
            Ok(PackResult { action: PackAction::Extracted, members, wrapper_removed: true, dest_dir: Some(dest_dir), renamed_to: None })
        };

        run().inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&temp_dir);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Fake {
        entries: Mutex<HashMap<PathBuf, Vec<String>>>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl PackDecompressor for Fake {
        fn list_entries(&self, file: &Path) -> Result<Vec<String>> {
            Ok(self.entries.lock().unwrap().get(file).cloned().unwrap_or_default())
        }

        fn extract_entries(&self, _file: &Path, out_dir: &Path, entries: &[String]) -> Result<()> {
            self.calls.lock().unwrap().push(entries.to_vec());
            for e in entries {
                let target = out_dir.join(e);
                std::fs::create_dir_all(target.parent().unwrap())?;
                std::fs::write(target, format!("stub:{e}"))?;
            }
            Ok(())
        }
    }

    struct Throwing;

    impl PackDecompressor for Throwing {
        fn list_entries(&self, _: &Path) -> Result<Vec<String>> {
            Err(crate::CoreError::Invalid("unsupported codec".into()))
        }

        fn extract_entries(&self, _: &Path, _: &Path, _: &[String]) -> Result<()> {
            panic!("should not be called")
        }
    }

    fn setup(entries: &[(&str, &[&str])]) -> (tempfile::TempDir, Arc<Fake>, PackExtractor) {
        let dir = tempfile::tempdir().unwrap();
        let fake = Arc::new(Fake::default());
        for (name, list) in entries {
            std::fs::write(dir.path().join(name), "wrapper-bytes").unwrap();
            fake.entries.lock().unwrap().insert(dir.path().join(name), list.iter().map(|s| s.to_string()).collect());
        }
        let ex = PackExtractor::new(fake.clone());
        (dir, fake, ex)
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        let mut n: Vec<String> = paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        n.sort();
        n
    }

    #[test]
    fn should_inspect_gates() {
        let (_d, _f, ex) = setup(&[]);
        assert!(ex.should_inspect(Path::new("/tmp/pack.zip"), SIZE_GATE_BYTES));
        assert!(ex.should_inspect(Path::new("/tmp/pack.zip"), SIZE_GATE_BYTES + 1));
        assert!(!ex.should_inspect(Path::new("/tmp/pack.zip"), SIZE_GATE_BYTES - 1));
        assert!(ex.should_inspect(Path::new("/tmp/weekly.cbz"), SIZE_GATE_BYTES));
        assert!(!ex.should_inspect(Path::new("/tmp/comic.pdf"), SIZE_GATE_BYTES * 4));
    }

    #[test]
    fn extracts_comic_members_into_a_pack_folder_and_drops_the_wrapper() {
        let (dir, fake, ex) = setup(&[("Weekly Pack.zip", &["Batman 001.cbz", "Batman 002.cbz", "readme.txt"])]);
        let wrapper = dir.path().join("Weekly Pack.zip");
        let r = ex.extract_pack(&wrapper, dir.path(), None).unwrap();
        assert_eq!(r.action, PackAction::Extracted);
        assert!(r.wrapper_removed && !wrapper.exists());
        assert!(dir.path().join("Weekly Pack").is_dir());
        assert_eq!(names(&r.members), ["Batman 001.cbz", "Batman 002.cbz"]);
        let mut called: Vec<String> = fake.calls.lock().unwrap().iter().flatten().cloned().collect();
        called.sort();
        assert_eq!(called, ["Batman 001.cbz", "Batman 002.cbz"]);
    }

    #[test]
    fn reports_progress_as_done_over_total() {
        let (dir, _f, ex) = setup(&[("pack.zip", &["a.cbz", "b.cbz", "c.cbz"])]);
        let seen = Mutex::new(vec![]);
        let cb = |d: usize, t: usize| seen.lock().unwrap().push((d, t));
        ex.extract_pack(&dir.path().join("pack.zip"), dir.path(), Some(&cb)).unwrap();
        let seen = seen.into_inner().unwrap();
        assert_eq!((seen[0], *seen.last().unwrap()), ((0, 3), (3, 3)));
    }

    #[test]
    fn renames_a_zip_of_loose_images_to_cbz() {
        let (dir, fake, ex) = setup(&[("Amazing Spider-Man 050.zip", &["001.jpg", "002.jpg", "003.jpg"])]);
        let wrapper = dir.path().join("Amazing Spider-Man 050.zip");
        let r = ex.extract_pack(&wrapper, dir.path(), None).unwrap();
        assert_eq!(r.action, PackAction::Renamed);
        assert_eq!(r.renamed_to, Some(dir.path().join("Amazing Spider-Man 050.cbz")));
        assert!(!wrapper.exists() && dir.path().join("Amazing Spider-Man 050.cbz").exists());
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn leaves_a_cbz_of_loose_images_untouched() {
        let (dir, fake, ex) = setup(&[("single.cbz", &["001.jpg", "002.jpg"])]);
        let wrapper = dir.path().join("single.cbz");
        assert_eq!(ex.extract_pack(&wrapper, dir.path(), None).unwrap().action, PackAction::Skipped);
        assert!(wrapper.exists() && fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn leaves_an_archive_without_comics_or_images_untouched() {
        let (dir, _f, ex) = setup(&[("mystery.zip", &["notes.txt", "cover.psd"])]);
        let wrapper = dir.path().join("mystery.zip");
        assert_eq!(ex.extract_pack(&wrapper, dir.path(), None).unwrap().action, PackAction::Skipped);
        assert!(wrapper.exists());
    }

    #[test]
    fn does_not_recurse_into_nested_archives_at_the_depth_cap() {
        let (dir, fake, ex) = setup(&[("nested.zip", &["inner.zip"])]);
        let r = ex.extract_pack_at(&dir.path().join("nested.zip"), dir.path(), None, 2).unwrap();
        assert_eq!(r.action, PackAction::Skipped);
        assert!(fake.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn dedupes_the_pack_folder_name() {
        let (dir, _f, ex) = setup(&[("Weekly Pack.zip", &["issue.cbz"])]);
        std::fs::create_dir(dir.path().join("Weekly Pack")).unwrap();
        let r = ex.extract_pack(&dir.path().join("Weekly Pack.zip"), dir.path(), None).unwrap();
        assert_eq!(r.dest_dir, Some(dir.path().join("Weekly Pack (2)")));
        assert!(dir.path().join("Weekly Pack (2)").join("issue.cbz").exists());
    }

    #[test]
    fn leaves_the_wrapper_when_listing_fails() {
        let dir = tempfile::tempdir().unwrap();
        let wrapper = dir.path().join("corrupt.zip");
        std::fs::write(&wrapper, "x").unwrap();
        let ex = PackExtractor::new(Arc::new(Throwing));
        assert_eq!(ex.extract_pack(&wrapper, dir.path(), None).unwrap().action, PackAction::Skipped);
        assert!(wrapper.exists());
    }

    #[test]
    fn nested_archives_are_unpacked_and_removed() {
        // outer.zip holds inner.zip, which holds two comics.
        struct Nested;
        impl PackDecompressor for Nested {
            fn list_entries(&self, file: &Path) -> Result<Vec<String>> {
                Ok(match file.file_name().unwrap().to_str().unwrap() {
                    "outer.zip" => vec!["inner.zip".into()],
                    "inner.zip" => vec!["A.cbz".into(), "B.cbz".into()],
                    _ => vec![],
                })
            }
            fn extract_entries(&self, _: &Path, out_dir: &Path, entries: &[String]) -> Result<()> {
                for e in entries {
                    std::fs::write(out_dir.join(e), "x")?;
                }
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let wrapper = dir.path().join("outer.zip");
        std::fs::write(&wrapper, "x").unwrap();
        let r = PackExtractor::new(Arc::new(Nested)).extract_pack(&wrapper, dir.path(), None).unwrap();
        assert_eq!(r.action, PackAction::Extracted);
        assert_eq!(names(&r.members), ["A.cbz", "B.cbz"]);
        assert!(!r.dest_dir.unwrap().join("inner.zip").exists());
    }
}
