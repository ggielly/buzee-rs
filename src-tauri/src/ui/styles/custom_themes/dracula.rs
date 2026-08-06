#![allow(clippy::unreadable_literal)]

use iced::color;

use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;

pub static DRACULA_DARK_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0x282a36),
        secondary: color!(0xff79c6),
        outgoing: color!(0x8be9fd),
        starred: color!(0xf1fa8c),
        text_headers: color!(0x282a36),
        text_body: color!(0xf8f8f2),
    });

pub static DRACULA_DARK_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| DRACULA_DARK_PALETTE.generate_palette_extension());

pub static DRACULA_LIGHT_PALETTE: std::sync::LazyLock<Palette> =
    std::sync::LazyLock::new(|| Palette {
        primary: color!(0xf8f8f2),
        secondary: color!(0x9f1670),
        outgoing: color!(0x005d6f),
        starred: color!(0xffb86c),
        text_headers: color!(0xf8f8f2),
        text_body: color!(0x282a36),
    });

pub static DRACULA_LIGHT_PALETTE_EXTENSION: std::sync::LazyLock<PaletteExtension> =
    std::sync::LazyLock::new(|| DRACULA_LIGHT_PALETTE.generate_palette_extension());
