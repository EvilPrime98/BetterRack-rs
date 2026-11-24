//! `GET /api/directories`: every folder under the download dir and library roots.
//! Directory listing under the library roots.

use crate::fs::{LibraryFs, StdFs};
use std::cmp::Ordering;

/// Roots first-seen order, each followed by its subfolders depth-first; duplicates dropped.
/// Unreadable folders are skipped silently.
pub fn directories_under(roots: &[String], cwd: &str) -> Vec<String> {
    directories_under_in(&StdFs, roots, cwd)
}

pub fn directories_under_in(fs: &dyn LibraryFs, roots: &[String], cwd: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !seen.contains(&s) {
            seen.push(s);
        }
    };
    for root in roots {
        let abs = crate::uid::resolve_in(root, cwd);
        push(abs.clone());
        let mut found = Vec::new();
        recurse(fs, &abs, &mut found);
        for dir in found {
            push(dir);
        }
    }
    seen
}

fn recurse(fs: &dyn LibraryFs, dir: &str, out: &mut Vec<String>) {
    let Ok(read) = fs.list_dir(dir) else {
        return;
    };
    let mut kids: Vec<_> = read.into_iter().filter(|e| e.is_dir).collect();
    kids.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for kid in kids {
        out.push(kid.path.clone());
        recurse(fs, &kid.path, out);
    }
}

/// Approximation of `localeCompare(b, undefined, { numeric: true, sensitivity: 'base' })`:
/// digit runs compare by value, everything else case-insensitively.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut s = String::new();
                    while let Some(c) = it.next_if(|c| c.is_ascii_digit()) {
                        s.push(c);
                    }
                    s.trim_start_matches('0').to_string()
                };
                let (n, m) = (take(&mut x), take(&mut y));
                let ord = n.len().cmp(&m.len()).then_with(|| n.cmp(&m));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(p), Some(q)) => {
                let ord = p.to_lowercase().cmp(q.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["Issue 10", "issue 2", "Issue 1", "Beta", "alpha"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["alpha", "Beta", "Issue 1", "issue 2", "Issue 10"]);
    }

    #[test]
    fn lists_roots_and_nested_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for d in ["b/nested", "a", "a/x"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("file.cbz"), b"").unwrap();
        let root_s = root.to_string_lossy().into_owned();
        let got = directories_under(
            &[
                root_s.clone(),
                root_s.clone(),
                "Z:\\definitely\\missing".into(),
            ],
            &root_s,
        );
        let rel: Vec<String> = got
            .iter()
            .map(|p| {
                p.strip_prefix(&crate::uid::resolve_in(&root_s, &root_s))
                    .unwrap_or(p)
                    .to_string()
            })
            .collect();
        assert_eq!(rel[0], "");
        assert_eq!(
            rel[1..5]
                .iter()
                .map(|s| s.trim_start_matches(['\\', '/']).replace('\\', "/"))
                .collect::<Vec<_>>(),
            ["a", "a/x", "b", "b/nested"]
        );
        assert!(got.last().unwrap().contains("missing"));
    }
}
