//! Entry uid, compatible with the Bun server's `LibraryModel.uidFromPath`.
//!
//! `uid = sha256(path.resolve(p))` as hex, sliced into a UUID-shaped string. Existing user data
//! (progress, ratings, identified metadata) is keyed by it, so the hashed string must match
//! Node's `path.resolve` byte for byte.

use sha2::{Digest, Sha256};
use std::path::Path;

const SEP: char = '\\';
const UNC: &str = r"\\";
const VERBATIM: &str = r"\\?\";

/// `sha256(abs_path)` formatted as `8-4-4-4-12` hex.
pub fn uid_from_path(abs_path: &str) -> String {
    let hex: String = Sha256::digest(abs_path.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Port of Node's `path.resolve(p)` for the Windows flavour: backslashes, `.`/`..` collapsed,
/// no trailing separator (except a bare root like `C:\`), no `\\?\` prefix, drive-letter case
/// kept as given. `cwd` is used for relative inputs.
pub fn resolve_windows(input: &str, cwd: &str) -> String {
    let input = input.strip_prefix(VERBATIM).unwrap_or(input);
    let unified = input.replace('/', r"\");
    let bytes = unified.as_bytes();
    let has_drive = bytes.len() >= 2 && bytes[1] == b':';
    let is_abs = unified.starts_with(UNC) || (has_drive && bytes.get(2) == Some(&b'\\'));
    let cwd_trim = cwd.trim_end_matches(SEP);
    let full = if is_abs {
        unified
    } else if has_drive {
        // Drive-relative (`C:foo`): best effort, relative to cwd.
        format!(r"{cwd_trim}\{}", &unified[2..])
    } else if let Some(rest) = unified.strip_prefix(SEP) {
        let drive = cwd.get(..2).unwrap_or("C:");
        format!(r"{drive}\{rest}")
    } else {
        format!(r"{cwd_trim}\{unified}")
    };

    let (root, rest) = if let Some(r) = full.strip_prefix(UNC) {
        // UNC: \\server\share\...
        let mut it = r.splitn(3, SEP);
        let server = it.next().unwrap_or("");
        let share = it.next().unwrap_or("");
        (format!(r"\\{server}\{share}\"), it.next().unwrap_or("").to_string())
    } else {
        (full[..3].to_string(), full[3..].to_string())
    };

    let mut parts: Vec<&str> = Vec::new();
    for seg in rest.split(SEP) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    let joined = parts.join(r"\");
    if joined.is_empty() {
        root
    } else {
        format!("{root}{joined}")
    }
}

/// Resolve a filesystem path the way the Bun server would on this platform.
pub fn resolve(path: &Path) -> String {
    #[cfg(windows)]
    {
        let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
        resolve_windows(&path.to_string_lossy(), &cwd)
    }
    #[cfg(not(windows))]
    {
        use std::path::Component;
        let mut out = std::path::PathBuf::new();
        for c in path.components() {
            match c {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                c => out.push(c),
            }
        }
        out.to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_is_uuid_shaped_and_stable() {
        let uid = uid_from_path(r"C:\Comics\a.cbz");
        assert_eq!(uid.len(), 36);
        assert_eq!(uid, uid_from_path(r"C:\Comics\a.cbz"));
        assert_ne!(uid, uid_from_path(r"C:\Comics\b.cbz"));
    }

    #[test]
    fn resolve_matches_node_path_resolve() {
        let cwd = r"C:\work";
        assert_eq!(resolve_windows(r"C:\Comics\", cwd), r"C:\Comics");
        assert_eq!(resolve_windows("C:/Comics/DC/../Marvel", cwd), r"C:\Comics\Marvel");
        assert_eq!(resolve_windows(r"\\?\D:\x\y", cwd), r"D:\x\y");
        assert_eq!(resolve_windows(r"sub\a.cbz", cwd), r"C:\work\sub\a.cbz");
        assert_eq!(resolve_windows(r"d:\Comics", cwd), r"d:\Comics");
        assert_eq!(resolve_windows(r"C:\", cwd), r"C:\");
        assert_eq!(resolve_windows(r"\\nas\share\a\", cwd), r"\\nas\share\a");
    }

    /// Real `(uid, path)` pairs captured from the running Bun server (`GET /api/library`).
    /// One `uid<TAB>path` per line in `tests/fixtures/uid_pairs.txt`; `#` lines are comments.
    #[test]
    fn uids_match_bun_server() {
        let data = include_str!("../tests/fixtures/uid_pairs.txt");
        for line in data.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let (uid, path) = line.split_once('\t').expect("uid<TAB>path");
            assert_eq!(uid_from_path(&resolve_windows(path, r"C:\")), uid, "path {path}");
        }
    }
}
