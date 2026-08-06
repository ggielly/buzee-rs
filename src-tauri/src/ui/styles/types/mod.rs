pub(super) mod color_remote;
pub mod custom_palette;
pub mod palette;
pub mod palette_extension;
pub mod style_type;

pub(crate) fn deserialize_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: serde::Deserialize<'de> + Default,
    D: serde::Deserializer<'de>,
{
    Ok(T::deserialize(deserializer).unwrap_or_default())
}
