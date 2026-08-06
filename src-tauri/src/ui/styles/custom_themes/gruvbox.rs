#![allow(clippy::unreadable_literal)]

use iced::color;

use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;

pub static GRUVBOX_DARK_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0x282828),
        secondary: color!(0xfe8019),
        outgoing: color!(0x8ec07c),
        starred: color!(0xd79921),
        text_headers: color!(0x1d2021),
        text_body: color!(0xebdbb2),
    });

pub static GRUVBOX_DARK_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| GRUVBOX_DARK_PALETTE.generate_palette_extension());

pub static GRUVBOX_LIGHT_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0xfbf1c7),
        secondary: color!(0xd65d0e),
        outgoing: color!(0x689d6a),
        starred: color!(0xd79921),
        text_headers: color!(0xf9f5d7),
        text_body: color!(0x282828),
    });

pub static GRUVBOX_LIGHT_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| GRUVBOX_LIGHT_PALETTE.generate_palette_extension());
