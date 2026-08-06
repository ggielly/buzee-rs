//! Ignore List screen: add folders/files to the ignore list and remove them.
//! Backed by the `IgnorePath` / `ShowIgnoredPaths` / `RemoveFromIgnoreList`
//! worker requests.

use crate::ui::message::Message;
use crate::ui::theme::{ButtonKind, ContainerKind, InputKind, TextKind, Theme};
use crate::ui::BuzeeApp;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{alignment, Alignment, Element, Length};

/// A brand-themed element.
type El<'a> = Element<'a, Message, Theme>;

pub fn view(app: &BuzeeApp) -> El<'_> {
    container(
        column![
            header(),
            scrollable(
                column![
                    add_form(app),
                    list(app),
                ]
                .spacing(20)
                .padding([24, 32])
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}

fn header<'a>() -> El<'a> {
    container(
        column![
            row![
                column![
                    text("Ignore List").size(24).class(TextKind::Default),
                    text("Folders and files excluded from the index")
                        .size(13)
                        .class(TextKind::Muted),
                ]
                .spacing(2),
                Space::new().width(Length::Fill),
                button(text("← Back to Settings").size(13).class(TextKind::Muted))
                    .on_press(crate::ui::message::Message::Navigate(crate::ui::state::Screen::Settings))
                    .class(ButtonKind::Ghost)
                    .padding([8, 16]),
            ]
            .align_y(Alignment::Center),
        ]
        .width(Length::Fill),
    )
    .padding(iced::Padding { top: 20.0, right: 32.0, bottom: 0.0, left: 32.0 })
    .into()
}

fn add_form<'a>(app: &'a BuzeeApp) -> El<'a> {
    let input = text_input("Type or paste a path to ignore…", &app.state.ignore_input)
        .on_input(Message::IgnoreInputChanged)
        .on_submit(Message::AddIgnorePath)
        .padding([10, 12])
        .size(14)
        .class(InputKind::Default)
        .width(Length::Fill);

    container(
        row![
            input,
            button(text("Add").size(13).class(TextKind::OnPrimary))
                .on_press(Message::AddIgnorePath)
                .class(ButtonKind::Primary)
                .padding([10, 22]),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::Fill)
    .into()
}

fn list<'a>(app: &'a BuzeeApp) -> El<'a> {
    let entries = &app.state.ignored_paths;
    if entries.is_empty() {
        return container(
            column![
                text("Nothing is ignored yet").size(15).class(TextKind::Default),
                text("Add a folder above to stop it from being indexed.")
                    .size(13)
                    .class(TextKind::Muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .padding(40)
        .class(ContainerKind::Card)
        .width(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .into();
    }

    let mut rows: Vec<El<'_>> = Vec::new();
    for entry in entries {
        let kind = if entry.is_folder { "folder" } else { "file" };
        let mode = if entry.ignore_indexing { "ignored completely" } else { "content ignored" };
        let row_el = container(
            row![
                column![
                    text(&entry.path).size(13).class(TextKind::Default),
                    text(format!("{kind}  •  {mode}")).size(11).class(TextKind::Muted),
                ]
                .spacing(2)
                .width(Length::Fill),
                button(text("Remove").size(12).class(TextKind::Purple))
                    .on_press(Message::RemoveIgnored(vec![entry.path.clone()]))
                    .class(ButtonKind::Ghost)
                    .padding([6, 12]),
            ]
            .align_y(Alignment::Center),
        )
        .padding([10, 0])
        .width(Length::Fill)
        .into();
        rows.push(row_el);
    }

    container(
        column![
            text(format!("{} ignored paths", entries.len())).size(13).class(TextKind::Purple),
            container(column(rows).spacing(2)).padding([12, 0]),
        ]
        .spacing(6),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::Fill)
    .into()
}
