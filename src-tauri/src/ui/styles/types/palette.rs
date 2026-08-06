use std::hash::{Hash, Hasher};
use std::path::Path;

use iced::Color;
use plotters::style::RGBColor;
use serde::{Deserialize, Serialize};

use super::color_remote::{deserialize_color, deserialize_color_inner, serialize_color};
use crate::ui::styles::style_constants::{RED_ALERT_COLOR_DAILY, RED_ALERT_COLOR_NIGHTLY};
use crate::ui::styles::types::color_remote::color_hash;
use crate::ui::styles::types::palette_extension::PaletteExtension;
use crate::ui::styles::types::style_type::StyleType;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Palette {
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub primary: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub secondary: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub outgoing: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub starred: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub text_headers: Color,
    #[serde(
        deserialize_with = "deserialize_color",
        serialize_with = "serialize_color"
    )]
    pub text_body: Color,
}

impl Palette {
    pub fn generate_buttons_color(self) -> Color {
        let primary = self.primary;
        let is_nightly = primary.r + primary.g + primary.b <= 1.5;
        if is_nightly {
            Color {
                r: f32::min(primary.r + 0.15, 1.0),
                g: f32::min(primary.g + 0.15, 1.0),
                b: f32::min(primary.b + 0.15, 1.0),
                a: 1.0,
            }
        } else {
            Color {
                r: f32::max(primary.r - 0.15, 0.0),
                g: f32::max(primary.g - 0.15, 0.0),
                b: f32::max(primary.b - 0.15, 0.0),
                a: 1.0,
            }
        }
    }

    pub fn generate_palette_extension(self) -> PaletteExtension {
        let primary = self.primary;
        let is_nightly = primary.r + primary.g + primary.b <= 1.5;
        let alpha_chart_badge = if is_nightly { 0.3 } else { 0.5 };
        let alpha_round_borders = if is_nightly { 0.3 } else { 0.6 };
        let alpha_round_containers = if is_nightly { 0.12 } else { 0.24 };
        let buttons_color = self.generate_buttons_color();
        let red_alert_color = if is_nightly {
            RED_ALERT_COLOR_NIGHTLY
        } else {
            RED_ALERT_COLOR_DAILY
        };

        PaletteExtension {
            is_nightly,
            alpha_chart_badge,
            alpha_round_borders,
            alpha_round_containers,
            buttons_color,
            red_alert_color,
        }
    }

    pub fn from_file<P>(path: P) -> Option<Self>
    where
        P: AsRef<Path>,
    {
        let toml_str = std::fs::read_to_string(path).ok()?;

        let toml: toml::Value = toml::from_str(&toml_str).ok()?;
        let primary = toml.get("primary").cloned()?;
        let secondary = toml.get("secondary").cloned()?;
        let outgoing = toml.get("outgoing").cloned()?;
        let starred = toml.get("starred").cloned()?;
        let text_headers = toml.get("text_headers").cloned()?;
        let text_body = toml.get("text_body").cloned()?;

        Some(Self {
            primary: deserialize_color_inner(primary)?,
            secondary: deserialize_color_inner(secondary)?,
            outgoing: deserialize_color_inner(outgoing)?,
            starred: deserialize_color_inner(starred)?,
            text_headers: deserialize_color_inner(text_headers)?,
            text_body: deserialize_color_inner(text_body)?,
        })
    }
}

impl Default for Palette {
    fn default() -> Self {
        <StyleType as std::default::Default>::default().get_palette()
    }
}

impl Hash for Palette {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Palette {
            primary,
            secondary,
            outgoing,
            starred,
            text_headers,
            text_body,
        } = self;

        color_hash(*primary, state);
        color_hash(*secondary, state);
        color_hash(*outgoing, state);
        color_hash(*starred, state);
        color_hash(*text_headers, state);
        color_hash(*text_body, state);
    }
}

pub fn to_rgb_color(color: Color) -> RGBColor {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    if color.r <= 1.0
        && color.r >= 0.0
        && color.g <= 1.0
        && color.g >= 0.0
        && color.b <= 1.0
        && color.b >= 0.0
    {
        RGBColor(
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
        )
    } else {
        RGBColor(0, 0, 0)
    }
}

pub fn mix_colors(color_1: Color, color_2: Color) -> Color {
    Color {
        r: f32::midpoint(color_1.r, color_2.r),
        g: f32::midpoint(color_1.g, color_2.g),
        b: f32::midpoint(color_1.b, color_2.b),
        a: 1.0,
    }
}
