//! Monochrome SVG icons (`assets/icons/*.svg`, copied from `react/src/icons`). Colour comes from
//! the element's `text_color`, like `currentColor` in the originals.
//!
//! Not SVG-able: `BetterRackIcon` (drawn natively in the header, it is a rounded square + "BR")
//! and `FandomIcon` (multicolour; GPUI `svg()` is single-colour, so the card shows a text label instead).
//! The star's half-fill (`StarIcon fill`) is a clip: layer two stars in the Rating component.

use gpui::{Pixels, Styled, Svg, svg};

#[allow(dead_code)] // FolderPlus/Menu/Wand/Xml arrive with later phases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    ArrowLeft,
    ArrowRight,
    BookOpen,
    Bookmark,
    Burger,
    ChevronDown,
    Close,
    Download,
    Folder,
    FolderPlus,
    Gear,
    Menu,
    Refresh,
    Search,
    Shop,
    Star,
    Trash,
    Wand,
    Xml,
    WinMinimize,
    WinMaximize,
    WinRestore,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Self::ArrowLeft => "icons/arrow-left.svg",
            Self::ArrowRight => "icons/arrow-right.svg",
            Self::BookOpen => "icons/book-open.svg",
            Self::Bookmark => "icons/bookmark.svg",
            Self::Burger => "icons/burger.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::Close => "icons/close.svg",
            Self::Download => "icons/download.svg",
            Self::Folder => "icons/folder.svg",
            Self::FolderPlus => "icons/folder-plus.svg",
            Self::Gear => "icons/gear.svg",
            Self::Menu => "icons/menu.svg",
            Self::Refresh => "icons/refresh.svg",
            Self::Search => "icons/search.svg",
            Self::Shop => "icons/shop.svg",
            Self::Star => "icons/star.svg",
            Self::Trash => "icons/trash.svg",
            Self::Wand => "icons/wand.svg",
            Self::Xml => "icons/xml.svg",
            Self::WinMinimize => "icons/win-minimize.svg",
            Self::WinMaximize => "icons/win-maximize.svg",
            Self::WinRestore => "icons/win-restore.svg",
        }
    }
}

/// A square icon of `size` px.
pub fn icon(icon: Icon, size: impl Into<Pixels>) -> Svg {
    let size = size.into();
    svg().path(icon.path()).size(size).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;
    use gpui::AssetSource;

    #[test]
    fn every_icon_asset_is_embedded() {
        use Icon::*;
        for i in [
            ArrowLeft, ArrowRight, BookOpen, Bookmark, Burger, ChevronDown, Close, Download, Folder,
            FolderPlus, Gear, Menu, Refresh, Search, Shop, Star, Trash, Wand, Xml, WinMinimize,
            WinMaximize, WinRestore,
        ] {
            assert!(Assets.load(i.path()).unwrap().is_some(), "missing {}", i.path());
        }
    }
}
