//! Groups issues of one series by cleaning the file name.

use regex::Regex;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static regex")
}

// JS `\b` and `\d` are ASCII-only, so those are spelled `(?-u:\b)` and `[0-9]` here.
static EXTENSION: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\.(?:cbz|cbr|cb7|cbt|zip|rar|7z|pdf)$"));
static TRAILING_GROUP: LazyLock<Regex> = LazyLock::new(|| re(r"\s*(?:\([^)]*\)|\[[^\]]*\])\s*$"));
static ANY_GROUP: LazyLock<Regex> = LazyLock::new(|| re(r"\([^)]*\)|\[[^\]]*\]"));
static YEAR_GROUP: LazyLock<Regex> = LazyLock::new(|| re(r"^[(\[]\s*((?:19|20)[0-9]{2})\s*[)\]]$"));
static TRAILING_ISSUE: LazyLock<Regex> = LazyLock::new(|| {
    re(concat!(
        r"(?i)[\s\-:,]*(?:(?:issue|no|nr|chapter|ch|episode|ep)\.?\s*|#\s*)?",
        r"[0-9]+(?:\.[0-9]+)?[a-z]?",
        r"(?:\s*[-–]\s*[0-9]+(?:\.[0-9]+)?[a-z]?)?",
        r"(?:\s+of\s+[0-9]+)?$",
    ))
});
static TRAILING_VOLUME: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)(?-u:\b)(?:volume|vol\.?|v)\s*[0-9]+$"));
static TRAILING_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"[\s\-–:#,]+$"));
static MARKS: LazyLock<Regex> = LazyLock::new(|| re(r"\p{M}+"));
static VOLUME_WORD: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?-u:\b)(?:volume|vol\.?|v)\s*0*([0-9]+)(?-u:\b)"));
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| re(r"[^\p{L}\p{N}]+"));
static LEADING_THE: LazyLock<Regex> = LazyLock::new(|| re(r"^the\s+(\S)"));
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| re(r"\s+"));

fn clean_base(raw: &str) -> String {
    let mut base = EXTENSION
        .replace(raw, "")
        .replace('_', " ")
        .trim()
        .to_string();
    if !base.chars().any(char::is_whitespace) {
        base = base.replace('.', " ");
    }
    while TRAILING_GROUP.is_match(&base) {
        base = TRAILING_GROUP.replace(&base, "").into_owned();
    }
    base = ANY_GROUP
        .replace_all(&base, |caps: &regex::Captures| {
            match YEAR_GROUP.captures(&caps[0]) {
                Some(y) => format!(" ({}) ", &y[1]),
                None => " ".to_string(),
            }
        })
        .into_owned();
    WHITESPACE.replace_all(&base, " ").trim().to_string()
}

/// `path.parse(name).name`: the file name without its last extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

pub struct Series {
    pub key: String,
    pub name: String,
}

/// `issue` is the identified comic's issue (`entry.identified ? entry.comic?.issue : undefined`).
pub fn series_of(file_name: &str, issue: Option<&str>) -> Series {
    let cleaned = clean_base(file_name);
    let mut base = cleaned.clone();

    if let Some(issue) = issue.map(str::trim).filter(|i| !i.is_empty())
        && let Ok(strip) = Regex::new(&format!(r"(?i)[\s\-:]*#?\s*{}$", regex::escape(issue)))
    {
        base = strip.replace(&base, "").into_owned();
    }
    if !TRAILING_VOLUME.is_match(&base) {
        base = TRAILING_ISSUE.replace(&base, "").into_owned();
    }
    base = TRAILING_PUNCT.replace(&base, "").trim().to_string();

    let name = if !base.is_empty() {
        base
    } else if !cleaned.is_empty() {
        cleaned
    } else {
        stem(file_name).to_string()
    };

    let folded: String = name.nfd().collect();
    let key = MARKS
        .replace_all(&folded, "")
        .to_lowercase()
        .replace('&', " and ");
    let key = VOLUME_WORD.replace_all(&key, "vol $1");
    let key = NON_ALNUM.replace_all(&key, " ");
    let key = LEADING_THE.replace(&key, "$1").trim().to_string();
    Series { key, name }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> String {
        series_of(name, None).key
    }

    #[test]
    fn ignores_the_comic_title_and_groups_by_file_name() {
        let a = series_of("Absolute Superman Vol 1 12.cbz", Some("12"));
        let b = series_of("Absolute Superman Vol 1 13.cbz", Some("13"));
        assert_eq!(a.key, b.key);
        assert_eq!(a.name, "Absolute Superman Vol 1");
    }

    #[test]
    fn groups_files_by_file_name() {
        assert_eq!(
            key("Absolute Superman Vol 1 12.cbz"),
            key("Absolute Superman Vol. 1 013 (2025).cbz")
        );
    }

    #[test]
    fn different_volumes_stay_separate() {
        assert_ne!(key("X Vol 1 3.cbz"), key("X Vol 2 3.cbz"));
    }

    #[test]
    fn handles_dotted_names_and_trailing_release_tags() {
        assert_eq!(
            key("Batman.012.cbz"),
            key("Batman 013 (Digital) [Group].cbz")
        );
    }

    #[test]
    fn handles_chapter_range_and_of_n_suffixes() {
        let k = key("Saga Vol 2 Ch 5.cbz");
        assert_eq!(k, "saga vol 2");
        assert_eq!(key("Saga Vol 2 12-13.cbz"), k);
        assert_eq!(key("Saga Vol 02 7 of 9.cbz"), k);
    }

    #[test]
    fn a_year_before_the_issue_separates_volumes_of_the_same_title() {
        assert_ne!(key("Batman (2016) 012.cbz"), key("Batman (2011) 012.cbz"));
        assert_eq!(
            key("Batman (2016) 012 (2017).cbz"),
            key("Batman (2016) 013.cbz")
        );
    }

    #[test]
    fn normalizes_accents_ampersands_and_leading_the() {
        assert_eq!(
            key("The Pokémon & Friends 1.cbz"),
            key("Pokemon and Friends 2.cbz")
        );
    }
}
