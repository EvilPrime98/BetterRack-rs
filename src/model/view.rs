//! Client-side enums and constants (react/src/library.types.ts), with the read-filter predicate.

use serde::{Deserialize, Serialize};

/// Card layout. Default `Detail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComicsType {
    Cover,
    #[default]
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadFilter {
    #[default]
    All,
    Read,
    Unread,
    Reading,
}

impl ReadFilter {
    /// `matchesReadFilter(filter, readPer)`.
    pub fn matches(self, read_per: f32) -> bool {
        match self {
            Self::All => true,
            Self::Read => read_per == 100.0,
            Self::Reading => read_per > 0.0 && read_per < 100.0,
            Self::Unread => read_per == 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FilterOption {
    #[default]
    #[serde(rename = "Alphabetically")]
    Alphabetically,
    #[serde(rename = "Creation Date")]
    CreationDate,
    #[serde(rename = "Release Date")]
    ReleaseDate,
}

impl FilterOption {
    pub fn label(self) -> &'static str {
        match self {
            Self::Alphabetically => "Alphabetically",
            Self::CreationDate => "Creation Date",
            Self::ReleaseDate => "Release Date",
        }
    }
}

impl ReadFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Read => "read",
            Self::Unread => "unread",
            Self::Reading => "reading",
        }
    }
}

impl ComicsType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Detail => "detail",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryStructure {
    #[default]
    Folders,
    Series,
}

/// "Recently added" window choices, in hours: day / week / month / year.
pub const RECENT_WINDOW_HOURS: [u32; 4] = [24, 168, 720, 8760];

/// Labels for [`RECENT_WINDOW_HOURS`] (`RECENT_WINDOW_OPTIONS` in the React source).
pub const RECENT_WINDOW_LABELS: [&str; 4] = ["Last 24 hours", "Last week", "Last month", "Last year"];

/// Downloads page poll interval.
pub const POLL_INTERVAL_MS: u64 = 750;
pub const IDENTIFY_POLL_INTERVAL_MS: u64 = 1000;
pub const DEFAULT_IMAGE_SIZE: u32 = 120;
/// Store posts per request; `hasMore = items.len() == STORE_PAGE_SIZE`.
pub const STORE_PAGE_SIZE: usize = 30;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_filter_matches() {
        assert!(ReadFilter::All.matches(37.0));
        assert!(ReadFilter::Read.matches(100.0));
        assert!(!ReadFilter::Read.matches(99.99));
        assert!(ReadFilter::Reading.matches(0.01));
        assert!(!ReadFilter::Reading.matches(0.0));
        assert!(!ReadFilter::Reading.matches(100.0));
        assert!(ReadFilter::Unread.matches(0.0));
    }

    #[test]
    fn filter_option_serializes_like_the_react_strings() {
        assert_eq!(serde_json::to_string(&FilterOption::CreationDate).unwrap(), "\"Creation Date\"");
    }
}
