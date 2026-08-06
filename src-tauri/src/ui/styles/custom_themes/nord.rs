#![allow(clippy::unreadable_literal)]

use iced::color;

use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;

pub static NORD_DARK_PALETTE: std::sync::LazyLock<Palette> = std::sync::LazyLock::new(|| Palette {
    primary: color!(0x2e3440),
    secondary: color!(0x88c0d0),
    outgoing: color!(0xB48EAD),
    starred: color!(0xebcb8b),
    text_headers: color!(0x2e3440),
    text_body: color!(0xd8dee9),
});

pub static NORD_DARK_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| NORD_DARK_PALETTE.generate_palette_extension());

pub static NORD_LIGHT_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0xeceff4),
        secondary: color!(0x05e81ac),
        outgoing: color!(0xb48ead),
        starred: color!(0xD08770),
        text_headers: color!(0xeceff4),
        text_body: color!(0x2e3440),
    });

pub static NORD_LIGHT_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| NORD_LIGHT_PALETTE.generate_palette_extension());
