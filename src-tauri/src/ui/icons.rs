//! Small brand glyphs and file-type badges rendered with iced primitives.
//!
//! Iced has no shipped icon font (lucide), so the lucide-style glyphs from the
//! original Svelte app are approximated with text glyphs, while the full-color
//! file-type badges are drawn as rounded chips carrying the extension.

use crate::ui::fonts;
use crate::ui::message::Message;
use crate::ui::theme::{ContainerKind, TextKind, Theme};
use iced::widget::svg::Handle;
use iced::widget::{container, svg};
use iced::{Color, Element, Length};

/// A colored file-type chip (e.g. "PDF" on a red rounded badge).
pub fn file_badge<'a>(file_type: &str, theme: &Theme) -> Element<'a, Message, Theme> {
    let ft = file_type.to_lowercase();
    let label = match ft.as_str() {
        "pdf" => "PDF",
        "docx" | "doc" => "DOC",
        "xlsx" | "xls" => "XLS",
        "csv" => "CSV",
        "pptx" | "ppt" => "PPT",
        "md" => "MD",
        "txt" => "TXT",
        "epub" => "EPUB",
        "mobi" => "MOBI",
        _ => "FILE",
    };
    let fill = theme.palette().filetype_color(&ft);
    container(text_badge(&label, 10))
        .width(Length::Fixed(38.0))
        .height(Length::Fixed(22.0))
        .class(ContainerKind::Badge(fill))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// Brand mark: the Buzee logo rendered as an inline SVG (purple rounded square
/// with a white "B").
pub fn logo_mark<'a>(size: f32) -> Element<'a, Message, Theme> {
    svg(Handle::from_memory(LOGO_SVG.as_bytes()))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

/// Inline SVG of the Buzee logo; drawn as paths so it needs no font support
/// from the SVG renderer.
const LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="2" y="2" width="96" height="96" rx="26" fill="#5368E0"/>
  <g fill="#ffffff">
    <rect x="27" y="25" width="14" height="50" rx="7"/>
    <path d="M41 25 h8 a13 13 0 0 1 0 26 h-8 z"/>
    <path d="M41 51 h11 a17 17 0 0 1 0 34 h-11 z"/>
  </g>
</svg>"##;

/// A plain monogram text badge (used inside file badges and elsewhere).
pub fn text_badge<'a>(text: &'a str, size: u16) -> iced::widget::Text<'a, Theme, iced::Renderer> {
    let mut t = iced::widget::text(text).size(size as f32);
    t = t.class(TextKind::White);
    t
}

/// A small text glyph with a given color (approximates a lucide icon).
pub fn glyph<'a>(symbol: &'a str, size: u16, color: Color) -> Element<'a, Message, Theme> {
    iced::widget::text(symbol)
        .size(size as f32)
        .class(TextKind::Color(color))
        .into()
}

/// A glyph drawn with the embedded icon font (`Icons for Sniffnet`).
pub fn icon<'a>(c: char, size: f32, color: Color) -> Element<'a, Message, Theme> {
    iced::widget::text(c.to_string())
        .font(fonts::ICONS)
        .size(size)
        .class(TextKind::Color(color))
        .into()
}
