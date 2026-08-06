//! Right-click context menu for the results table.
//!
//! A right-click anywhere over the results (tracked by the mouse subscription)
//! opens a small popover with actions for the currently-selected result. The
//! popover is rendered as the top layer of a `Stack`, with a transparent
//! full-area backdrop that dismisses the menu when clicking elsewhere.

use crate::ui::icons;
use crate::ui::message::Message;
use crate::ui::theme::{ButtonKind, ContainerKind, TextKind, Theme};
use crate::ui::BuzeeApp;
use iced::widget::{button, column, container, row, text, Stack};
use iced::{alignment, Alignment, Element, Length, Padding};

/// Wrap the results `content` in the context-menu overlay if one is open;
/// otherwise return the content unchanged.
pub fn overlay<'a>(
    app: &'a BuzeeApp,
    content: Element<'a, Message, Theme>,
) -> Element<'a, Message, Theme> {
    let Some((x, y)) = app.state.context_menu else {
        return content;
    };

    let path = app
        .state
        .selected_result
        .and_then(|id| app.state.results.iter().find(|r| r.id == id))
        .map(|r| r.path.clone());
    let Some(path) = path else {
        return content;
    };

    let palette = app.theme().unwrap().palette();

    let menu_item = |glyph: char, label: &'a str, msg: Message| -> Element<'a, Message, Theme> {
        button(
            row![
                icons::icon(glyph, 14.0, palette.foreground),
                text(label).size(13).class(TextKind::Default),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(msg)
        .class(ButtonKind::Ghost)
        .width(Length::Fill)
        .padding([8, 12])
        .into()
    };

    let menu = container(
        column![
            menu_item('o', "Open", Message::OpenResult(path.clone())),
            menu_item('8', "Reveal in folder", Message::RevealResult(path)),
        ]
        .spacing(2)
        .width(Length::Fill),
    )
    .width(Length::Fixed(190.0))
    .padding(4)
    .class(ContainerKind::Popover);

    // Position the popover so its top-left corner sits at the cursor: the
    // container fills the layer and its padding shifts the content.
    let positioned = container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding { top: y, left: x, bottom: 0.0, right: 0.0 })
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top);

    // Invisible click-catcher: clicking anywhere outside the menu closes it.
    let backdrop =
        iced::widget::button::<Message, Theme, iced::Renderer>(iced::widget::Space::new())
            .on_press(Message::CloseContextMenu)
            .class(ButtonKind::Ghost)
            .width(Length::Fill)
            .height(Length::Fill);

    Stack::<Message, Theme, iced::Renderer>::new()
        .width(Length::Fill)
        .height(Length::Fill)
        .extend([content, backdrop.into(), positioned.into()])
        .into()
}
