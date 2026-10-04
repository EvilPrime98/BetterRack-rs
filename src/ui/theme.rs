//! Design tokens. Dark theme only.
//!
//! The `oklab()` backgrounds were converted to sRGB once (CSS Color 4 matrices):
//! `oklab(0.298136 …)` → #2c2d32, `oklab(0.32 …)` → #313238 (the window title bar colour), `oklab(0.27 …)` → #25262b.

use gpui::{Pixels, Rgba, px, rgb, rgba};

pub fn accent() -> Rgba {
    rgb(0x34c3d1)
}
/// rgba(52,195,209,0.35)
pub fn accent_soft() -> Rgba {
    rgba(0x34c3d159)
}
pub fn bg_app() -> Rgba {
    rgb(0x2c2d32)
}
pub fn bg_chrome() -> Rgba {
    rgb(0x313238)
}
pub fn bg_panel() -> Rgba {
    rgb(0x25262b)
}
pub fn bg_canvas() -> Rgba {
    rgb(0x141414)
}
pub fn bg_modal() -> Rgba {
    rgb(0x1c1c1c)
}
/// rgba(255,255,255,0.08)
pub fn border_subtle() -> Rgba {
    rgba(0xffffff14)
}
#[allow(dead_code)] // design token
pub fn scrollbar_thumb() -> Rgba {
    rgb(0x4a4a4a)
}
/// Toast error border.
pub fn error() -> Rgba {
    rgb(0xff5c5c)
}
pub fn text() -> Rgba {
    rgb(0xffffff)
}
/// The `#c7c7c7` the icons default to.
pub fn text_muted() -> Rgba {
    rgb(0xc7c7c7)
}
/// Hover wash used on header/window buttons.
pub fn hover() -> Rgba {
    rgba(0xffffff14)
}

pub fn header_height() -> Pixels {
    px(56.0)
}
pub fn sidebar_width() -> Pixels {
    px(280.0)
}
#[allow(dead_code)] // design token
pub fn content_max_width() -> Pixels {
    px(1360.0)
}

/// Geist, embedded in `assets/fonts` and registered by [`register_fonts`] at startup.
pub const FONT_SANS: &str = "Geist";
pub const FONT_MONO: &str = "Geist Mono";

/// Weights the design uses: Geist Sans 400/500/600/700, Geist Mono 500/600 (`main.css`).
const FONT_FILES: [&str; 6] = [
    "fonts/Geist-Regular.ttf",
    "fonts/Geist-Medium.ttf",
    "fonts/Geist-SemiBold.ttf",
    "fonts/Geist-Bold.ttf",
    "fonts/GeistMono-Medium.ttf",
    "fonts/GeistMono-SemiBold.ttf",
];

/// Register the embedded fonts with GPUI's text system. A missing or unreadable file only costs
/// that weight (GPUI falls back to a system font), so it is logged, not fatal.
pub fn register_fonts(cx: &mut gpui::App) {
    use gpui::AssetSource as _;
    let fonts = FONT_FILES
        .iter()
        .filter_map(|path| match crate::assets::Assets.load(path) {
            Ok(Some(data)) => Some(data),
            _ => {
                tracing::warn!("embedded font {path} is missing");
                None
            }
        })
        .collect();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        tracing::warn!("could not register the embedded fonts: {e}");
    }
}
