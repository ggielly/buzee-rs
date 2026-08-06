use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::ui::styles::types::deserialize_or_default;
use crate::ui::styles::types::palette::Palette;
use crate::ui::styles::types::palette_extension::PaletteExtension;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CustomPalette {
    #[serde(flatten)]
    #[serde(deserialize_with = "deserialize_or_default")]
    pub(crate) palette: Palette,
    #[serde(flatten)]
    #[serde(deserialize_with = "deserialize_or_default")]
    pub(crate) extension: PaletteExtension,
}

impl CustomPalette {
    pub fn from_palette(palette: Palette) -> Self {
        Self {
            palette,
            extension: palette.generate_palette_extension(),
        }
    }
}
