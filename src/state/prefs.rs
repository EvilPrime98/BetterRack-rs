//! User preferences, comic types, sidebar and library-structure state. Client-only, persisted in
//! `<config dir>/BetterRack/prefs.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::{ComicsType, FilterOption, LibraryStructure, ReadFilter};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub filter: FilterOption,
    pub comic_type: ComicsType,
    pub ask_stop_downloads: bool,
    pub zoom: f32,
    pub structure: LibraryStructure,
    pub sidebar_compact: bool,
    pub sidebar_collapsed: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            filter: FilterOption::Alphabetically,
            comic_type: ComicsType::Detail,
            ask_stop_downloads: true,
            zoom: 1.0,
            structure: LibraryStructure::Folders,
            sidebar_compact: false,
            sidebar_collapsed: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("BetterRack").join("prefs.json"))
}

impl Prefs {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Best effort: losing a preference is not worth an error dialog.
    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if let Err(e) = std::fs::write(&p, json) {
                tracing::warn!("could not save prefs to {}: {e}", p.display());
            }
        }
    }
}

/// Entity wrapper: persisted [`Prefs`] plus the session-only read filter (`ReadTypesContext`).
pub struct PrefsStore {
    pub prefs: Prefs,
    pub read_filter: ReadFilter,
    /// Id of the header dropdown that is open, if any (one at a time).
    pub open_menu: Option<&'static str>,
}

impl PrefsStore {
    pub fn load() -> Self {
        Self {
            prefs: Prefs::load(),
            read_filter: ReadFilter::All,
            open_menu: None,
        }
    }

    /// Mutate a persisted preference and write it out.
    pub fn update(&mut self, cx: &mut gpui::Context<Self>, change: impl FnOnce(&mut Prefs)) {
        change(&mut self.prefs);
        self.prefs.save();
        cx.notify();
    }

    pub fn set_read_filter(&mut self, value: ReadFilter, cx: &mut gpui::Context<Self>) {
        self.read_filter = value;
        cx.notify();
    }

    pub fn set_comics_type(&mut self, value: ComicsType, cx: &mut gpui::Context<Self>) {
        self.update(cx, |p| p.comic_type = value);
    }

    pub fn set_menu_open(&mut self, id: &'static str, open: bool, cx: &mut gpui::Context<Self>) {
        if open {
            self.open_menu = Some(id);
        } else if self.open_menu == Some(id) {
            self.open_menu = None;
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_files_fall_back_to_defaults() {
        let p: Prefs = serde_json::from_str(r#"{"comicType":"cover"}"#).unwrap();
        assert_eq!(p.comic_type, ComicsType::Cover);
        assert!(p.ask_stop_downloads);
        assert_eq!(p.zoom, 1.0);
    }
}
