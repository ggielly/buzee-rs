//! Misc screens: the "Extract Text" utility and the "Tips & Shortcuts" page.

use crate::ui::message::Message;
use crate::ui::theme::{ButtonKind, ContainerKind, InputKind, TextKind, Theme};
use crate::ui::BuzeeApp;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{alignment, Alignment, Element, Length};

/// A brand-themed element.
type El<'a> = Element<'a, Message, Theme>;

// ---------------------------------------------------------------------------
// Extract Text
// ---------------------------------------------------------------------------

pub fn extract_view(app: &BuzeeApp) -> El<'_> {
    let input = text_input("/path/to/document.pdf", &app.state.extract_input)
        .on_input(crate::ui::message::Message::ExtractInputChanged)
        .on_submit(crate::ui::message::Message::RunExtractText)
        .padding([10, 12])
        .size(14)
        .class(InputKind::Default)
        .width(Length::Fill);

    let form = container(
        column![
            row![
                input,
                button(text("Extract").size(13).class(TextKind::OnPrimary))
                    .on_press(crate::ui::message::Message::RunExtractText)
                    .class(ButtonKind::Primary)
                    .padding([10, 22]),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        ]
        .width(Length::Fill),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::Fill);

    let output: El<'_> = match &app.state.extract_output {
        Some(text_content) => container(
            scrollable(text(text_content).size(13).class(TextKind::Default))
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .padding(16)
        .class(ContainerKind::Muted)
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        None => container(
            column![
                text("No output yet").size(15).class(TextKind::Default),
                text("Enter a document path above to extract its text content.")
                    .size(13)
                    .class(TextKind::Muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .into(),
    };

    container(
        column![
            text("Extract Text").size(24).class(TextKind::Default),
            text("Pull the plain-text content out of a document")
                .size(13)
                .class(TextKind::Muted),
            form,
            output,
        ]
        .spacing(16)
        .padding([24, 32])
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Tips & Shortcuts
// ---------------------------------------------------------------------------

pub fn tips_view<'a>(_app: &'a BuzeeApp) -> El<'a> {
    let tip = |title: &'a str, body: &'a str| -> El<'a> {
        container(
            column![
                text(title).size(14).class(TextKind::Default),
                text(body).size(13).class(TextKind::Muted),
            ]
            .spacing(4)
            .width(Length::Fill),
        )
        .padding([14, 0])
        .width(Length::Fill)
        .into()
    };

    let shortcut = |keys: &'a str, action: &'a str| -> El<'a> {
        container(
            row![
                container(text(keys).size(12).class(TextKind::Purple))
                    .padding([4, 10])
                    .class(ContainerKind::Muted),
                Space::new().width(Length::Fill),
                text(action).size(13).class(TextKind::Default),
            ]
            .align_y(Alignment::Center),
        )
        .padding([8, 0])
        .width(Length::Fill)
        .into()
    };

    container(
        column![
            text("Tips & Shortcuts").size(24).class(TextKind::Default),
            text("Search like a pro with these pointers").size(13).class(TextKind::Muted),
            scrollable(
                column![
                    container(
                        column![
                            text("Search tips").size(13).class(TextKind::Purple),
                            tip("Be specific", "last year \"annual report\" -pdf finds the annual report PDF from last year."),
                            tip("Use the date filter", "Narrow results with the date-range picker above the results list."),
                            tip("Filter by file type", "Pick PDF, Word, Excel or PowerPoint from the file-type dropdown."),
                            tip("Search locations", "Look inside a specific folder, recent files or bookmarks."),
                        ]
                        .spacing(6),
                    )
                    .padding(20)
                    .class(ContainerKind::Card)
                    .width(Length::Fill),
                    container(
                        column![
                            text("Keyboard shortcuts").size(13).class(TextKind::Purple),
                            shortcut("Alt+Space", "Focus the search bar from anywhere"),
                            shortcut("Ctrl+K", "Focus the search bar"),
                            shortcut("Enter", "Run the search"),
                            shortcut("Esc", "Clear the search"),
                        ]
                        .spacing(6),
                    )
                    .padding(20)
                    .class(ContainerKind::Card)
                    .width(Length::Fill),
                ]
                .spacing(16),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(16)
        .padding([24, 32])
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}
