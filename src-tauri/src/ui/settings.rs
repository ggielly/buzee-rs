//! Settings screen: preference toggles persisted through the user-preferences
//! service, OCR numeric options, the global shortcut, rescan/clear actions and
//! a link to the ignore list.

use crate::ui::message::{BoolPref, Message};
use crate::ui::state::Screen;
use crate::ui::theme::{
    ButtonKind, ContainerKind, InputKind, PickKind, SliderKind, TextKind, Theme, TogglerKind,
};
use crate::ui::BuzeeApp;
use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, text, text_input, toggler,
    Space,
};
use iced::{Alignment, Element, Length};

/// A brand-themed element.
type El<'a> = Element<'a, Message, Theme>;

/// A styled on/off switch (the built-in iced `toggler`).
fn toggle<'a>(value: bool, on_toggle: impl Fn(bool) -> Message + 'a) -> El<'a> {
    toggler(value).on_toggle(on_toggle).class(TogglerKind::Default).into()
}

pub fn view(app: &BuzeeApp) -> El<'_> {
    let prefs = &app.state.preferences;

    let search_section = section(
        "Search & Indexing",
        column![
            setting_row(
                "Search suggestions",
                "Show suggestions as you type in the search bar",
                toggle(prefs.show_search_suggestions, |v| {
                    Message::SetBoolPref(BoolPref::SearchSuggestions, v)
                }),
            ),
            setting_row(
                "Detailed scan",
                "Scan file contents for deeper search results",
                toggle(prefs.detailed_scan, |v| {
                    Message::SetBoolPref(BoolPref::DetailedScan, v)
                }),
            ),
            setting_row(
                "Parse PDFs",
                "Extract text from PDF files when indexing",
                toggle(prefs.parse_pdfs, |v| {
                    Message::SetBoolPref(BoolPref::ParsePdfs, v)
                }),
            ),
            ocr_settings(app),
        ],
    );

    let sync_section = section(
        "Sync",
        column![
            setting_row(
                "Automatic background sync",
                "Keep the index up to date in the background",
                toggle(prefs.automatic_background_sync, |v| {
                    Message::SetBoolPref(BoolPref::AutomaticBackgroundSync, v)
                }),
            ),
            setting_row(
                "Launch at startup",
                "Start Buzee automatically when you log in",
                toggle(prefs.launch_at_startup, |v| {
                    Message::SetBoolPref(BoolPref::LaunchAtStartup, v)
                }),
            ),
        ],
    );

    let shortcut_section = section(
        "Global Shortcut",
        column![
            setting_row(
                "Enable global shortcut",
                "Open Buzee from anywhere",
                toggle(prefs.global_shortcut_enabled, |v| {
                    Message::SetBoolPref(BoolPref::GlobalShortcutEnabled, v)
                }),
            ),
            shortcut_row(app),
        ],
    );

    let danger_section = section(
        "Index",
        column![
            rescan_row(app),
            setting_row(
                "Enable logs",
                "Write debug logs to disk",
                toggle(prefs.enable_logs, |v| {
                    Message::SetBoolPref(BoolPref::EnableLogs, v)
                }),
            ),
            setting_row(
                "Clear the index",
                "Remove all indexed files and start fresh",
                button(text("Clear Index").size(13).class(TextKind::White))
                    .on_press(Message::ClearIndex)
                    .class(ButtonKind::Danger)
                    .padding([8, 18])
                    .into(),
            ),
            setting_row(
                "Ignore list",
                "Manage folders and files excluded from the index",
                button(text("Manage →").size(13).class(TextKind::Purple))
                    .on_press(Message::Navigate(Screen::Ignore))
                    .class(ButtonKind::Ghost)
                    .padding([8, 18])
                    .into(),
            ),
        ],
    );

    let appearance_section = section(
        "Appearance",
        column![
            setting_row(
                "UI theme",
                "Color theme of the interface",
                theme_picker(app),
            ),
        ],
    );

    container(
        column![
            header_row(),
            scrollable(
                column![
                    appearance_section,
                    search_section,
                    sync_section,
                    shortcut_section,
                    danger_section,
                ]
                .spacing(22)
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

fn header_row<'a>() -> El<'a> {
    container(
        column![
            text("Settings").size(24).class(TextKind::Default),
            text("Tune how Buzee indexes and syncs your files")
                .size(13)
                .class(TextKind::Muted),
        ]
        .spacing(2),
    )
    .width(Length::Fill)
    .padding([20, 32])
    .into()
}

fn section<'a, T>(title: &'a str, body: T) -> El<'a>
where
    T: Into<Element<'a, Message, Theme>>,
{
    container(
        column![
            text(title).size(13).class(TextKind::Purple),
            container(body).padding([16, 0]).width(Length::Fill),
        ]
        .spacing(6),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::Fill)
    .into()
}

fn setting_row<'a>(label: &'a str, desc: &'a str, control: El<'a>) -> El<'a> {
    container(
        row![
            column![
                text(label).size(14).class(TextKind::Default),
                text(desc).size(12).class(TextKind::Muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            control,
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding([10, 0])
    .width(Length::Fill)
    .into()
}

fn ocr_settings<'a>(app: &'a BuzeeApp) -> El<'a> {
    let pages = app.state.ocr_pages_input.parse::<i32>().unwrap_or(150).clamp(1, 5000);
    let threads = app.state.ocr_threads_input.parse::<i32>().unwrap_or(1).clamp(1, 4);

    let pages_slider = slider(1..=5000, pages, |v| {
        Message::OcrPagesInputChanged(v.to_string())
    })
    .step(10)
    .class(SliderKind::Default)
    .width(Length::Fill);

    let threads_slider = slider(1..=4, threads, |v| {
        Message::OcrThreadsInputChanged(v.to_string())
    })
    .step(1)
    .class(SliderKind::Default)
    .width(Length::Fill);

    let orders = vec![
        "size_asc".to_string(),
        "size_desc".to_string(),
        "pages_asc".to_string(),
        "pages_desc".to_string(),
    ];
    let order = pick_list(
        orders,
        Some(app.state.preferences.ocr_sort_order.clone()),
        Message::SetOcrSortOrder,
    )
    .placeholder("Sort order")
    .class(PickKind::Default)
    .padding([6, 10])
    .width(Length::Fixed(180.0));

    container(
        column![
            row![
                column![
                    text("Max OCR pages").size(14).class(TextKind::Default),
                    text("Pages parsed per PDF during OCR").size(12).class(TextKind::Muted),
                ]
                .spacing(2)
                .width(Length::Fill),
                text(pages.to_string()).size(14).class(TextKind::Default),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
            pages_slider,
            row![
                column![
                    text("OCR threads").size(14).class(TextKind::Default),
                    text("Parallel OCR worker threads (1-4)").size(12).class(TextKind::Muted),
                ]
                .spacing(2)
                .width(Length::Fill),
                text(threads.to_string()).size(14).class(TextKind::Default),
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .padding([10, 0]),
            threads_slider,
            row![
                column![
                    text("OCR sort order").size(14).class(TextKind::Default),
                    text("Order in which documents are parsed").size(12).class(TextKind::Muted),
                ]
                .spacing(2)
                .width(Length::Fill),
                order,
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .padding([10, 0]),
            row![
                container(text("Changes apply after saving").size(11).class(TextKind::Muted)),
                Space::new().width(Length::Fill),
                button(text("Save").size(13).class(TextKind::OnPrimary))
                    .on_press(Message::SaveOcrNumbers)
                    .class(ButtonKind::Primary)
                    .padding([8, 18]),
            ]
            .align_y(Alignment::Center)
            .padding([10, 0]),
        ]
        .width(Length::Fill),
    )
    .padding([10, 0])
    .width(Length::Fill)
    .into()
}

fn shortcut_row<'a>(app: &'a BuzeeApp) -> El<'a> {
    let input = text_input("Alt+Space", &app.state.shortcut_input)
        .on_input(Message::ShortcutInputChanged)
        .padding([8, 10])
        .size(14)
        .class(InputKind::Default)
        .width(Length::Fixed(160.0));

    container(
        row![
            column![
                text("Shortcut").size(14).class(TextKind::Default),
                text("Combination to focus the search bar").size(12).class(TextKind::Muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            input,
            button(text("Save").size(13).class(TextKind::OnPrimary))
                .on_press(Message::SaveShortcut)
                .class(ButtonKind::Primary)
                .padding([8, 18]),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 0])
    .width(Length::Fill)
    .into()
}

fn rescan_row<'a>(app: &'a BuzeeApp) -> El<'a> {
    let _scanning = app.state.statistics.as_ref().map(|s| s.status == "running").unwrap_or(false);
    container(
        row![
            column![
                text("Rescan documents").size(14).class(TextKind::Default),
                text("Re-index changed or new files on disk").size(12).class(TextKind::Muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            button(text("Rescan new").size(13).class(TextKind::Default))
                .on_press(Message::RescanDocuments(false))
                .class(ButtonKind::Muted)
                .padding([8, 18]),
            button(text("Rescan all").size(13).class(TextKind::Purple))
                .on_press(Message::RescanDocuments(true))
                .class(ButtonKind::Purple)
                .padding([8, 18]),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 0])
    .width(Length::Fill)
    .into()
}

fn theme_picker<'a>(app: &'a BuzeeApp) -> El<'a> {
    let current = app
        .state
        .themes
        .iter()
        .find(|choice| choice.theme == app.state.theme)
        .cloned();
    pick_list(app.state.themes.clone(), current, Message::ThemeSelected)
        .placeholder("Theme")
        .class(PickKind::Default)
        .padding([6, 10])
        .width(Length::Fixed(200.0))
        .into()
}
