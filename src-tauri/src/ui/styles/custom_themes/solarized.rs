#![allow(clippy::unreadable_literal)]

use iced::color;

use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;

pub static SOLARIZED_LIGHT_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0xfdf6e3),
        secondary: color!(0x859900),
        outgoing: color!(0x268bd2),
        starred: color!(0xb58900),
        text_headers: color!(0xfdf6e3),
        text_body: color!(0x002b36),
    });

pub static SOLARIZED_LIGHT_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| SOLARIZED_LIGHT_PALETTE.generate_palette_extension());

pub static SOLARIZED_DARK_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0x002b36),
        secondary: color!(0x859900),
        outgoing: color!(0x268bd2),
        starred: color!(0xb58900),
        text_headers: color!(0x002b36),
        text_body: color!(0xeee8d5),
    });

pub static SOLARIZED_DARK_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| SOLARIZED_DARK_PALETTE.generate_palette_extension());
