//! Embedded fonts for the UI.
//!
//! The app is a plain iced binary (no Tauri bundling), so fonts are compiled in
//! via `include_bytes!` and registered with iced at boot, then referenced by
//! their font-family name. The subset fonts were copied from Sniffnet:
//!   - `Sarasa Mono SC for Sniffnet`: the default body font (renders the app's
//!     text/UI glyphs reliably, including wide CJK coverage).
//!   - `Icons for Sniffnet`: a vector icon font used for brand/UI glyphs.

use iced::Font;

/// Family name of the body font (matches the TTF `name` table of the subset).
pub const FONT_FAMILY_NAME: &str = "Sarasa Mono SC for Sniffnet";

/// Family name of the icon font (matches the TTF `name` table of `icons.ttf`).
pub const ICON_FONT_FAMILY_NAME: &str = "Icons for Sniffnet";

/// Compiled-in bytes of the Sarasa Mono SC subset used as the default font.
pub const BODY_BYTES: &[u8] =
    include_bytes!("../../../ressources/fonts/subset/sarasa-mono-sc-regular.subset.ttf");

/// Compiled-in bytes of the Sniffnet icon font.
pub const ICONS_BYTES: &[u8] = include_bytes!("../../../ressources/fonts/subset/icons.ttf");

/// Default body font.
pub const BODY: Font = Font::with_name(FONT_FAMILY_NAME);

/// The vector icon font.
pub const ICONS: Font = Font::with_name(ICON_FONT_FAMILY_NAME);