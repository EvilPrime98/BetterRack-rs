//! 7-Zip-backed formats (RAR4/RAR5, solid or not).
//! Skipped when no 7-Zip is installed.

use br_core::archive::{Archives, resolve_seven_zip};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn rar_fixtures_list_read_and_expose_comic_info() {
    if resolve_seven_zip(
        std::env::var_os("SEVEN_ZIP_PATH")
            .map(PathBuf::from)
            .as_deref(),
    )
    .is_err()
    {
        eprintln!("skipped: 7-Zip not found");
        return;
    }
    let archives = Archives::new(std::env::var_os("SEVEN_ZIP_PATH").map(PathBuf::from));
    for name in ["rar4.cbr", "rar4-solid.cbr", "rar5.cbr", "rar5-solid.cbr"] {
        let path = fixture(name);
        // `010` sorts after `002` numerically; nested files and ComicInfo.xml are not pages.
        assert_eq!(
            archives.list_pages(&path).unwrap(),
            ["001.jpg", "002.jpg", "010.jpg"],
            "{name}"
        );
        assert!(
            !archives.read_entry(&path, "001.jpg").unwrap().is_empty(),
            "{name}"
        );
        assert!(
            archives.comic_info(&path).unwrap().is_some(),
            "{name}: top-level ComicInfo.xml"
        );
        assert!(archives.read_entry(&path, "missing.jpg").is_err(), "{name}");
    }
}
