//! Process configuration shared by every entry point (server binary, future in-process backend).
//!
//! Env contract (shared with `platform/server_process.rs`):
//! - `PORT`: default 3000, `0` lets the OS pick.
//! - `BR_API_KEY`: when set, `/api/*` and `/read/*` require it (header `x-br-api-key` or `?key=`).
//! - `LOG_LEVEL`: tracing filter, default `info`.
//! - `SEVEN_ZIP_PATH`: path to the bundled 7-Zip executable.
//! - `BR_DATA_DIR`: the base directory for data files. Defaults to the process cwd.
//!
//! The SQLite files live in `<data_dir>/src/database/`, to keep existing data
//! (dev checkout and packaged sidecar alike, where the base is `%APPDATA%\BetterRack`) usable
//! as is.

use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 3000;
pub const DEFAULT_LOG_LEVEL: &str = "info";
/// Relative to the data dir.
pub const DB_SUBDIR: &str = "src/database";
pub const COMIC_DATA_DB: &str = "comic-data.sqlite";
pub const PREFERENCES_DB: &str = "preferences.sqlite";
pub const JOBS_DB: &str = "jobs.sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub api_key: Option<String>,
    pub log_level: String,
    pub seven_zip_path: Option<PathBuf>,
    pub data_dir: PathBuf,
}

impl Config {
    /// Build from the process environment.
    pub fn from_env() -> std::io::Result<Self> {
        let cwd = std::env::current_dir()?;
        Ok(Self::from_lookup(|k| std::env::var(k).ok(), cwd))
    }

    /// Build from an arbitrary lookup, so tests do not touch the real environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>, cwd: PathBuf) -> Self {
        let non_empty = |k: &str| get(k).filter(|v| !v.is_empty());
        Self {
            // Unset or empty -> 3000, otherwise the parsed value. Unparseable falls back too.
            port: non_empty("PORT")
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_PORT),
            api_key: non_empty("BR_API_KEY"),
            log_level: non_empty("LOG_LEVEL").unwrap_or_else(|| DEFAULT_LOG_LEVEL.into()),
            seven_zip_path: non_empty("SEVEN_ZIP_PATH").map(PathBuf::from),
            data_dir: non_empty("BR_DATA_DIR").map(PathBuf::from).unwrap_or(cwd),
        }
    }

    /// Folder holding the three `.sqlite` files.
    pub fn db_dir(&self) -> PathBuf {
        self.data_dir.join(DB_SUBDIR)
    }

    pub fn comic_data_db(&self) -> PathBuf {
        self.db_dir().join(COMIC_DATA_DB)
    }

    pub fn preferences_db(&self) -> PathBuf {
        self.db_dir().join(PREFERENCES_DB)
    }

    pub fn jobs_db(&self) -> PathBuf {
        self.db_dir().join(JOBS_DB)
    }
}

/// Constant-time comparison for API keys (length leak only).
pub fn keys_match(provided: &str, required: &str) -> bool {
    let (a, b) = (provided.as_bytes(), required.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Config {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|k| m.get(k).cloned(), PathBuf::from("cwd"))
    }

    #[test]
    fn defaults() {
        let c = cfg(&[]);
        assert_eq!(c.port, 3000);
        assert_eq!(c.api_key, None);
        assert_eq!(c.log_level, "info");
        assert_eq!(c.seven_zip_path, None);
        assert_eq!(c.data_dir, PathBuf::from("cwd"));
    }

    #[test]
    fn empty_values_count_as_unset() {
        let c = cfg(&[("PORT", ""), ("BR_API_KEY", ""), ("BR_DATA_DIR", "")]);
        assert_eq!(
            (c.port, c.api_key, c.data_dir),
            (3000, None, PathBuf::from("cwd"))
        );
    }

    #[test]
    fn port_zero_is_kept_and_garbage_falls_back() {
        assert_eq!(cfg(&[("PORT", "0")]).port, 0);
        assert_eq!(cfg(&[("PORT", "abc")]).port, 3000);
    }

    #[test]
    fn db_files_follow_the_bun_layout() {
        let c = cfg(&[("BR_DATA_DIR", "data")]);
        assert_eq!(
            c.comic_data_db(),
            PathBuf::from("data").join("src/database/comic-data.sqlite")
        );
        assert_eq!(
            c.preferences_db(),
            PathBuf::from("data").join("src/database/preferences.sqlite")
        );
        assert_eq!(
            c.jobs_db(),
            PathBuf::from("data").join("src/database/jobs.sqlite")
        );
    }

    #[test]
    fn api_key_comparison() {
        assert!(keys_match("secret", "secret"));
        assert!(!keys_match("secret", "secreT"));
        assert!(!keys_match("sec", "secret"));
    }
}
