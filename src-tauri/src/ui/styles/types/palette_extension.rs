use std::hash::{Hash, Hasher};

use iced::Color;
use serde::{Deserialize, Serialize};

use super::color_remote::{deserialize_color, serialize_color};
use crate::ui::styles::types::color_remote::color_hash;
use crate::ui::styles::types::deserialize_or_default;
use crate::ui::styles::types::style_type::StyleType;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaletteExtension {
    #[serde(deserialize_with = "deserialize_or_default")]
    pub is_nightly: bool,
    #[serde(deserialize_with = "deserialize_or_default")]
    pub alpha_chart_badge: f32,
    #[serde(deserialize_with = "deserialize_or_default")]
    pub alpha_round_borders: f32,
    #[serde(deserialize_with = "deserialize_or_default")]
    pub alpha_round_containers: f32,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub buttons_color: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub red_alert_color: Color,
}

impl Default for PaletteExtension {
    fn default() -> Self {
        <StyleType as std::default::Default>::default().get_extension()
    }
}

impl Hash for PaletteExtension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let PaletteExtension {
            is_nightly,
            alpha_chart_badge,
            alpha_round_borders,
            alpha_round_containers,
            buttons_color,
            red_alert_color,
        } = self;

        is_nightly.hash(state);
        #[allow(clippy::cast_possible_truncation)]
        (997 * (alpha_chart_badge + alpha_round_borders + alpha_round_containers) as i32)
            .hash(state);
        color_hash(*buttons_color, state);
        color_hash(*red_alert_color, state);
    }
}
