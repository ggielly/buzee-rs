use std::hash::{Hash, Hasher};

use iced::Color;
use serde::{Deserialize, Deserializer, Serializer};

const HEX_STR_BASE_LEN: usize = 7;
const HEX_STR_ALPHA_LEN: usize = 9;

#[allow(clippy::unnecessary_wraps)]
pub(super) fn deserialize_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(deserialize_color_inner(deserializer).unwrap_or(Color::BLACK))
}

pub(super) fn deserialize_color_inner<'de, D>(deserializer: D) -> Option<Color>
where
    D: Deserializer<'de>,
{
    let hex = String::deserialize(deserializer).ok()?;

    let hex_len = hex.len();
    if hex_len == HEX_STR_BASE_LEN || hex_len == HEX_STR_ALPHA_LEN {
        let digits_str = hex.strip_prefix('#')?;

        let r_str = digits_str.get(0..2)?;
        let g_str = digits_str.get(2..4)?;
        let b_str = digits_str.get(4..6)?;
        let a_str = digits_str.get(6..8).unwrap_or("ff");

        let r = u8::from_str_radix(r_str, 16).ok()?;
        let g = u8::from_str_radix(g_str, 16).ok()?;
        let b = u8::from_str_radix(b_str, 16).ok()?;
        let a = u8::from_str_radix(a_str, 16).ok()?;

        Some(Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        })
    } else {
        None
    }
}

#[inline]
pub(super) fn color_hash<H: Hasher>(color: Color, state: &mut H) {
    let color = color.into_rgba8();
    color.hash(state);
}

#[inline]
pub(super) fn serialize_color<S>(color: &Color, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let color = color.into_rgba8();

    let hex_color = if color[3] == 255 {
        format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color[0], color[1], color[2], color[3]
        )
    };

    serializer.serialize_str(&hex_color)
}
