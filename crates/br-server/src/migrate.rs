//! `br-server migrate-downloaded [--apply]`: record library comics that match a store post.
//!
//! Run with the server stopped; a running server keeps `downloaded.json` in memory and would
//! overwrite the migrated file.

use crate::state::AppState;
use br_core::library::scan;
use std::collections::BTreeSet;
use std::path::Path;

/// Lowercase alphanumeric words: extension, bracketed groups and `vol`/`volume` words dropped,
/// leading zeros of numbers removed.
pub fn normalize_title(raw: &str) -> String {
    let mut text = String::new();
    let mut depth = 0usize;
    for c in raw.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => text.push(c),
            _ => {}
        }
    }
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| w != "vol" && w != "volume")
        .map(|w| {
            if w.chars().all(|c| c.is_ascii_digit()) {
                let t = w.trim_start_matches('0');
                if t.is_empty() {
                    "0".to_string()
                } else {
                    t.to_string()
                }
            } else {
                w
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn stem(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
}

pub async fn run(state: &AppState, apply: bool) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = state.config.data_dir.to_string_lossy().into_owned();
    let roots = state.prefs.get_app_settings()?.output_dirs;
    let comics: Vec<_> = scan(&roots, &cwd).into_iter().filter(|e| !e.did).collect();
    let snapshot = state.downloaded.snapshot();
    let recorded: BTreeSet<&str> = snapshot.paths.values().map(String::as_str).collect();

    let (mut matched, mut skipped, mut already) = (0usize, 0usize, 0usize);
    for comic in &comics {
        if recorded.contains(comic.path.as_str()) {
            already += 1;
            continue;
        }
        let title = normalize_title(stem(&comic.name));
        if title.is_empty() {
            skipped += 1;
            println!("skipped (empty title): {}", comic.path);
            continue;
        }
        let posts = match state.store.get_post_links(&title, "1", Some("20")).await {
            Ok(p) => p,
            Err(e) => {
                skipped += 1;
                println!("skipped (store error: {e}): {}", comic.path);
                continue;
            }
        };
        let ids: BTreeSet<i64> = posts
            .iter()
            .filter(|p| normalize_title(&p.title) == title)
            .filter_map(|p| p.id)
            .collect();
        match ids.iter().next() {
            Some(&id) if ids.len() == 1 => {
                matched += 1;
                println!("match: post {id} <- {}", comic.path);
                if apply {
                    state.downloaded.fill(id, Path::new(&comic.path));
                }
            }
            Some(_) => {
                skipped += 1;
                println!("skipped (ambiguous, {} posts): {}", ids.len(), comic.path);
            }
            None => {
                skipped += 1;
                println!("skipped (no store match): {}", comic.path);
            }
        }
    }
    println!(
        "{} {matched}, skipped {skipped}, already recorded {already}",
        if apply {
            "applied:"
        } else {
            "dry-run, matched"
        }
    );
    if !apply {
        println!("re-run with --apply to write downloaded.json");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_titles() {
        assert_eq!(normalize_title("Batman #001 (2026) [Digital]"), "batman 1");
        assert_eq!(normalize_title("Saga: Vol. 02 - Part_1"), "saga 2 part 1");
        assert_eq!(normalize_title("  "), "");
        assert_eq!(normalize_title("Mañana 000"), "mañana 0");
    }
}
