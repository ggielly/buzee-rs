use crate::ui::styles::custom_themes::a11y::{
    A11Y_DARK_PALETTE, A11Y_DARK_PALETTE_EXTENSION, A11Y_LIGHT_PALETTE,
    A11Y_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::custom_themes::dracula::{
    DRACULA_DARK_PALETTE, DRACULA_DARK_PALETTE_EXTENSION, DRACULA_LIGHT_PALETTE,
    DRACULA_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::custom_themes::gruvbox::{
    GRUVBOX_DARK_PALETTE, GRUVBOX_DARK_PALETTE_EXTENSION, GRUVBOX_LIGHT_PALETTE,
    GRUVBOX_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::custom_themes::nord::{
    NORD_DARK_PALETTE, NORD_DARK_PALETTE_EXTENSION, NORD_LIGHT_PALETTE,
    NORD_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::custom_themes::solarized::{
    SOLARIZED_DARK_PALETTE, SOLARIZED_DARK_PALETTE_EXTENSION, SOLARIZED_LIGHT_PALETTE,
    SOLARIZED_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::custom_themes::yeti::{
    YETI_DARK_PALETTE, YETI_DARK_PALETTE_EXTENSION, YETI_LIGHT_PALETTE,
    YETI_LIGHT_PALETTE_EXTENSION,
};
use crate::ui::styles::types::custom_palette::CustomPalette;
use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Hash, PartialEq, Default)]
#[serde(tag = "style", content = "attributes")]
#[allow(clippy::large_enum_variant)]
pub enum StyleType {
    #[default]
    A11yDark,
    A11yLight,
    DraculaDark,
    DraculaLight,
    GruvboxDark,
    GruvboxLight,
    NordDark,
    NordLight,
    SolarizedDark,
    SolarizedLight,
    YetiDark,
    YetiLight,
    Custom(CustomPalette),
}

impl StyleType {
    pub fn get_palette(self) -> Palette {
        match self {
            Self::A11yDark => *A11Y_DARK_PALETTE,
            Self::A11yLight => *A11Y_LIGHT_PALETTE,
            Self::DraculaDark => *DRACULA_DARK_PALETTE,
            Self::DraculaLight => *DRACULA_LIGHT_PALETTE,
            Self::GruvboxDark => *GRUVBOX_DARK_PALETTE,
            Self::GruvboxLight => *GRUVBOX_LIGHT_PALETTE,
            Self::NordDark => *NORD_DARK_PALETTE,
            Self::NordLight => *NORD_LIGHT_PALETTE,
            Self::SolarizedDark => *SOLARIZED_DARK_PALETTE,
            Self::SolarizedLight => *SOLARIZED_LIGHT_PALETTE,
            Self::YetiDark => *YETI_DARK_PALETTE,
            Self::YetiLight => *YETI_LIGHT_PALETTE,
            Self::Custom(custom_palette) => custom_palette.palette,
        }
    }

    pub fn get_extension(self) -> PaletteExtension {
        match self {
            Self::A11yDark => *A11Y_DARK_PALETTE_EXTENSION,
            Self::A11yLight => *A11Y_LIGHT_PALETTE_EXTENSION,
            Self::DraculaDark => *DRACULA_DARK_PALETTE_EXTENSION,
            Self::DraculaLight => *DRACULA_LIGHT_PALETTE_EXTENSION,
            Self::GruvboxDark => *GRUVBOX_DARK_PALETTE_EXTENSION,
            Self::GruvboxLight => *GRUVBOX_LIGHT_PALETTE_EXTENSION,
            Self::NordDark => *NORD_DARK_PALETTE_EXTENSION,
            Self::NordLight => *NORD_LIGHT_PALETTE_EXTENSION,
            Self::SolarizedDark => *SOLARIZED_DARK_PALETTE_EXTENSION,
            Self::SolarizedLight => *SOLARIZED_LIGHT_PALETTE_EXTENSION,
            Self::YetiDark => *YETI_DARK_PALETTE_EXTENSION,
            Self::YetiLight => *YETI_LIGHT_PALETTE_EXTENSION,
            Self::Custom(custom_palette) => custom_palette.extension,
        }
    }

    pub const fn all_styles() -> &'static [Self] {
        &[
            Self::A11yDark,
            Self::A11yLight,
            Self::DraculaDark,
            Self::DraculaLight,
            Self::GruvboxDark,
            Self::GruvboxLight,
            Self::NordDark,
            Self::NordLight,
            Self::SolarizedDark,
            Self::SolarizedLight,
            Self::YetiDark,
            Self::YetiLight,
        ]
    }
}

impl fmt::Display for StyleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::A11yDark => write!(f, "A11y Dark"),
            Self::A11yLight => write!(f, "A11y Light"),
            Self::DraculaDark => write!(f, "Dracula Dark"),
            Self::DraculaLight => write!(f, "Dracula Light"),
            Self::GruvboxDark => write!(f, "Gruvbox Dark"),
            Self::GruvboxLight => write!(f, "Gruvbox Light"),
            Self::NordDark => write!(f, "Nord Dark"),
            Self::NordLight => write!(f, "Nord Light"),
            Self::SolarizedDark => write!(f, "Solarized Dark"),
            Self::SolarizedLight => write!(f, "Solarized Light"),
            Self::YetiDark => write!(f, "Yeti Dark"),
            Self::YetiLight => write!(f, "Yeti Light"),
            Self::Custom(_) => write!(f, "Custom"),
        }
    }
}
