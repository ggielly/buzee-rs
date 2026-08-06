//! The full Search screen, ported pixel-faithfully from the original Svelte +
//! Tailwind/shadcn layout:
//!   - a 36px title bar (brand mark + name, window controls),
//!   - a header row with the search bar and sync-status button,
//!   - a left sidebar (20vw),
//!   - a filter row (Location / File type / Date range / layout toggles),
//!   - the results table (List) or icon grid (Grid), plus the empty state,
//!   - a bottom status bar spanning the full width.

use crate::ui::icons::logo_mark;
use crate::ui::message::Message;
use crate::ui::state::{ScanPhase, Screen, ViewMode};
use crate::ui::theme::{ButtonKind, ContainerKind, InputKind, PickKind, TextKind, Theme};
use crate::ui::BuzeeApp;
use chrono::{Local, TimeZone};
use iced::widget::{
    button, column, container, pick_list, progress_bar, row, rule, scrollable, text,
    text_input, Space,
};
use iced::{alignment, Alignment, Color, Element, Length};

/// A brand-themed element.
type El<'a> = Element<'a, Message, Theme>;

/// Truncate a string to at most `max` characters, ellipsizing the tail.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Build the root element of the application. The title bar, global header and
/// status bar are always present; the sidebar + content swap per screen.
pub fn root(app: &BuzeeApp) -> El<'_> {
    // Show the full-window progress popup only for user-triggered scans.
    if app.state.scan_popup_active && (app.state.scan_running || app.state.scan_complete) {
        return scan_popup(app);
    }

    let content = match app.state.screen {
        Screen::Dashboard => crate::ui::dashboard::view(app),
        Screen::Search => main_content(app),
        Screen::Settings => crate::ui::settings::view(app),
        Screen::Ignore => crate::ui::ignore::view(app),
        Screen::ExtractText => crate::ui::screens::extract_view(app),
        Screen::Tips => crate::ui::screens::tips_view(app),
    };

    column![
        title_bar(),
        header(app),
        row![sidebar(app), content].spacing(0).height(Length::Fill),
        status_bar(app),
    ]
    .height(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Scan progress popup (full-window modal while a scan is running).
// ---------------------------------------------------------------------------
fn scan_popup(app: &BuzeeApp) -> El<'_> {
    let s = &app.state;
    let running = s.scan_running;

    let (phase_title, phase_hint) = if !running {
        ("Scan complete", "Your files are now indexed and searchable.")
    } else {
        match s.scan_phase {
            ScanPhase::Idle => ("Preparing scan", "Gathering the file list…"),
            ScanPhase::Scanning => ("Scanning files", "Walking the directory tree…"),
            ScanPhase::Parsing => ("Indexing content", "Extracting text from your documents…"),
            ScanPhase::Ocr => ("OCR rescan", "Recognizing text in PDFs and images…"),
        }
    };

    let total = s.scan_total.max(1);
    let processed = s.scan_processed.min(total);
    let ratio = if running || s.scan_total > 0 {
        (processed as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let bar = progress_bar::<Theme>(0.0..=1.0, ratio)
        .length(Length::Fill)
        .girth(Length::Fixed(8.0));

    let stats_line = if running {
        format!(
            "{} of {} files processed  •  {} failed",
            processed, total, s.scan_failed
        )
    } else {
        format!(
            "{} files processed  •  {} failed  •  {} files added",
            processed, s.scan_failed, s.scan_files_added
        )
    };

    let current_file = if running && !s.scan_current_file.is_empty() {
        text(truncate(&s.scan_current_file, 80))
            .size(12)
            .class(TextKind::Muted)
    } else {
        text(" ").size(12).class(TextKind::Muted)
    };

    let failed_list: El<'_> = if !s.scan_items.is_empty() {
        let items = s.scan_items.iter().map(|item| {
            let row: El<'_> = row![
                text("✕").size(11).class(TextKind::Error),
                text(truncate(&item.name, 52))
                    .size(11)
                    .class(TextKind::Error),
            ]
            .spacing(6)
            .into();
            row
        });
        column![
            text(format!("Failed files ({})", s.scan_items.len()))
                .size(12)
                .class(TextKind::Default),
            scrollable(column(items).spacing(4)).height(Length::Fixed(90.0)),
        ]
        .spacing(6)
        .width(Length::Fill)
        .into()
    } else if running {
        text("No errors so far").size(12).class(TextKind::Muted).into()
    } else {
        text("No errors").size(12).class(TextKind::Muted).into()
    };

    let action = if running {
        button(text("Cancel").size(14).class(TextKind::OnPrimary))
            .on_press(Message::StopSync)
            .class(ButtonKind::Primary)
            .padding([10, 28])
    } else {
        button(text("Done").size(14).class(TextKind::OnPrimary))
            .on_press(Message::DismissScanPopup)
            .class(ButtonKind::Primary)
            .padding([10, 28])
    };

    let card = container(
        column![
            row![logo_mark(28.0), text("Buzee").size(16).class(TextKind::Default)]
                .spacing(10)
                .align_y(Alignment::Center),
            text(phase_title).size(20).class(TextKind::Default),
            text(phase_hint).size(13).class(TextKind::Muted),
            Space::new().height(6),
            bar,
            text(stats_line).size(13).class(TextKind::Muted),
            current_file,
            Space::new().height(4),
            failed_list,
            Space::new().height(8),
            action,
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(440.0))
    .padding(28)
    .class(ContainerKind::Card);

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center)
        .class(ContainerKind::Fill(Color::from_rgba8(2, 8, 23, 0.62)))
        .into()
}

// ---------------------------------------------------------------------------
// Title bar (h-9 = 36px)
// ---------------------------------------------------------------------------
fn title_bar<'a>() -> El<'a> {
    let brand = row![
        logo_mark(16.0),
        text("Buzee").size(14).class(TextKind::Default),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let controls = row![
        window_button("min"),
        window_button("max"),
        window_button("×"),
    ]
    .spacing(0);

    container(
        row![brand, Space::new().width(Length::Fill), controls].align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(36.0))
    .class(ContainerKind::TitleBar)
    .padding(iced::Padding { top: 0.0, right: 4.0, bottom: 0.0, left: 12.0 })
    .into()
}

fn window_button<'a>(label: &'a str) -> El<'a> {
    let is_close = label == "×";
    let kind = if is_close { ButtonKind::Danger } else { ButtonKind::Ghost };
    let color = if is_close { TextKind::White } else { TextKind::Muted };
    button(text(label).size(14).class(color))
        .on_press(if is_close { Message::Close } else { Message::Noop })
        .class(kind)
        .width(Length::Fixed(40.0))
        .height(Length::Fixed(26.0))
        .into()
}

// ---------------------------------------------------------------------------
// Header: search bar + sync-status button
// ---------------------------------------------------------------------------
fn header(app: &BuzeeApp) -> El<'_> {
    let search = text_input(
        "Search documents, files, folders...",
        app.state.search_input.as_str(),
    )
    .on_input(Message::SearchInputChanged)
    .on_submit(Message::RunSearch)
    .padding(12)
    .size(16)
    .class(InputKind::Search)
    .width(Length::Fill);

    let sync = button(
        row![
            text("↻").size(16).class(TextKind::Purple),
            text("Sync").size(14).class(TextKind::Default),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(Message::ToggleSync)
    .class(ButtonKind::Ghost)
    .padding([10, 16]);

    container(
        column![
            row![search, sync].spacing(12).align_y(Alignment::Center),
            search_suggestions(app),
        ]
        .spacing(6),
    )
    .width(Length::Fill)
    .padding([14, 24])
    .class(ContainerKind::Panel)
    .into()
}

/// Inline suggestions dropdown shown below the search bar once the user pauses
/// while typing (mirrors the original command palette). Hidden by default.
fn search_suggestions(app: &BuzeeApp) -> El<'_> {
    let show = app.state.preferences.show_search_suggestions
        && app.state.search_input.trim().chars().count() >= 3
        && !app.state.suggestions.is_empty();

    if !show {
        return Space::new().height(0).into();
    }

    let palette = app.theme().unwrap().palette();
    let entries = app.state.suggestions.iter().map(|suggestion| {
        let entry: El<'_> = button(
            row![
                crate::ui::icons::icon('5', 12.0, palette.muted_foreground),
                text(suggestion.clone()).size(13).class(TextKind::Default),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Message::SearchSuggestionSelected(suggestion.clone()))
        .class(ButtonKind::Ghost)
        .width(Length::Fill)
        .padding([8, 12])
        .into();
        entry
    });

    container(scrollable(column(entries).spacing(2).width(Length::Fill)))
        .width(Length::Fill)
        .max_height(280.0)
        .class(ContainerKind::Popover)
        .into()
}

// ---------------------------------------------------------------------------
// Sidebar (width ~20%). Active item follows the current screen.
// ---------------------------------------------------------------------------
fn sidebar<'a>(app: &'a BuzeeApp) -> El<'a> {
    let item = |icon_char: char, label: &'a str, screen: Screen| -> El<'a> {
        let active = app.state.screen == screen;
        let p = app.theme().unwrap().palette();
        let color = if active { p.purple } else { p.muted_foreground };
        iced::widget::button::<Message, Theme, iced::Renderer>(
            row![
                crate::ui::icons::icon(icon_char, 16.0, color),
                text(label).size(14).class(if active { TextKind::Purple } else { TextKind::Muted }),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Navigate(screen))
        .class(if active { ButtonKind::Purple } else { ButtonKind::Ghost })
        .width(Length::Fill)
        .padding([9, 12])
        .into()
    };

    // Glyphs are codepoints from the embedded icon font (Sniffnet's
    // "Icons for Sniffnet"): Overview, Inspect, File, Settings, Lightning.
    let items = column![
        item('d', "Dashboard", Screen::Dashboard),
        item('5', "Search", Screen::Search),
        item('8', "Extract Text", Screen::ExtractText),
        item('a', "Settings", Screen::Settings),
        item('z', "Tips & Shortcuts", Screen::Tips),
    ]
    .spacing(2)
    .padding([12, 10]);

    let footer = container(text("Buzee  •  v0.2").size(11).class(TextKind::Muted))
        .padding([12, 10]);

    container(column![items, footer].spacing(8).height(Length::Fill))
        .width(Length::FillPortion(1))
        .height(Length::Fill)
        .class(ContainerKind::Sidebar)
        .into()
}

// ---------------------------------------------------------------------------
// Main column: filters + results (+ empty state).
// ---------------------------------------------------------------------------
fn main_content(app: &BuzeeApp) -> El<'_> {
    column![
        filter_bar(app),
        rule::horizontal(1),
        results_area(app),
    ]
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}

fn filter_bar(app: &BuzeeApp) -> El<'_> {
    let locations = vec![
        "my computer".to_string(),
        "recent".to_string(),
        "bookmarks".to_string(),
    ];
    let file_types = vec![
        "All types".to_string(),
        "PDF".to_string(),
        "Word".to_string(),
        "Excel".to_string(),
        "PowerPoint".to_string(),
        "Text".to_string(),
        "Markdown".to_string(),
    ];
    let ranges = date_presets();
    let range_labels: Vec<String> = ranges.iter().map(|r| r.0.clone()).collect();

    let location = pick_list(
        locations,
        Some(app.state.location.clone()),
        Message::LocationChanged,
    )
    .placeholder("Location")
    .class(PickKind::Default)
    .padding([6, 10])
    .width(Length::Fixed(170.0));

    let selected_type = app.state.file_type.clone().unwrap_or_else(|| "All types".to_string());
    let filetype = pick_list(
        file_types,
        Some(selected_type),
        |v: String| {
            let ft = if v == "All types" { None } else { Some(v) };
            Message::FileTypeChanged(ft)
        },
    )
    .placeholder("File type")
    .class(PickKind::Default)
    .padding([6, 10])
    .width(Length::Fixed(170.0));

    let current_range = app.state.date_range.as_ref().map(|r| r.0.clone());
    let range = pick_list(
        range_labels,
        current_range,
        move |v: String| {
            let found = ranges.iter().find(|r| r.0 == v).cloned();
            Message::DateRangeChanged(found)
        },
    )
    .placeholder("Date range")
    .class(PickKind::Default)
    .padding([6, 10])
    .width(Length::Fixed(180.0));

    let layout_toggles = view_toggle_bar(app);

    row![
        location,
        filetype,
        range,
        layout_toggles,
    ]
    .spacing(16)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .padding([12, 24])
    .into()
}

fn view_toggle_bar<'a>(app: &'a BuzeeApp) -> El<'a> {
    let seg = |label: &'a str, msg: Message, active: bool| -> El<'a> {
        let kind = if active { ButtonKind::Primary } else { ButtonKind::Ghost };
        let color = if active { TextKind::OnPrimary } else { TextKind::Muted };
        iced::widget::button::<Message, Theme, iced::Renderer>(
            iced::widget::text::<Theme, iced::Renderer>(label).size(13).class(color),
        )
        .on_press(msg)
        .class(kind)
        .width(Length::Fixed(64.0))
        .padding([8, 0])
        .into()
    };

    let is_list = app.state.view_mode == ViewMode::List;

    row![
        separator_label("Layout  "),
        seg("List", Message::ViewModeChanged(ViewMode::List), is_list),
        seg("Grid", Message::ViewModeChanged(ViewMode::Grid), !is_list),
        seg(
            "=Compact",
            Message::CompactChanged(!app.state.compact_view),
            app.state.compact_view && is_list,
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

fn separator_label<'a>(label: &'a str) -> El<'a> {
    row![
        container(container(text(" ").size(1)).width(Length::Fixed(1.0)).height(Length::Fixed(20.0)))
            .width(Length::Fixed(1.0))
            .class(ContainerKind::Muted),
        text(label).size(13).class(TextKind::Muted),
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .into()
}

// ---------------------------------------------------------------------------
// Results area
// ---------------------------------------------------------------------------
fn results_area(app: &BuzeeApp) -> El<'_> {
    let empty_query = app.state.search_input.trim().is_empty();

    if empty_query && app.state.results.is_empty() {
        return hints_state();
    }
    if app.state.results.is_empty() {
        return no_results_state();
    }

    match app.state.view_mode {
        ViewMode::List => {
            let table = crate::ui::result_table::view(app);
            crate::ui::context_menu::overlay(app, table)
        }
        ViewMode::Grid => grid(app),
    }
}

fn hints_state<'a>() -> El<'a> {
    container(
        column![
            text("Start typing to search your files").size(18).class(TextKind::Default),
            text("e.g.  annual report  2023").size(14).class(TextKind::Muted),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center)
    .into()
}

fn no_results_state<'a>() -> El<'a> {
    let tips = button(text("View all tips and shortcuts").size(13).class(TextKind::Purple))
        .on_press(Message::Navigate(Screen::Tips))
        .class(ButtonKind::Purple)
        .padding([6, 12]);

    container(
        column![
            logo_mark(64.0),
            text("No Results").size(20).class(TextKind::Default),
            text("Try modifying your query? You can be more specific like –")
                .size(13)
                .class(TextKind::Muted),
            text("last year \"annual report\" -pdf").size(13).class(TextKind::Purple),
            tips,
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(alignment::Horizontal::Center)
    .align_y(alignment::Vertical::Center)
    .into()
}

// ---------------------------------------------------------------------------
// Grid view: simple icon grid
// ---------------------------------------------------------------------------
fn grid(app: &BuzeeApp) -> El<'_> {
    let mut per_row: Vec<El<'_>> = Vec::new();
    let mut rows: Vec<El<'_>> = Vec::new();
    for res in &app.state.results {
        let label = if res.name.is_empty() { &res.path } else { &res.name };
        let tile = button(
            column![
                logo_mark(40.0),
                text(label).size(11).class(TextKind::Default).width(Length::Fixed(96.0)),
                text(&res.file_type).size(10).class(TextKind::Muted),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .on_press(Message::OpenResult(res.path.clone()))
        .class(ButtonKind::Ghost)
        .padding(10)
        .into();
        per_row.push(tile);
        if per_row.len() >= 5 {
            let group: Vec<El<'_>> = per_row.drain(..).collect();
            rows.push(row(group).spacing(8).into());
        }
    }
    if !per_row.is_empty() {
        rows.push(row(per_row).spacing(8).into());
    }

    scrollable(column(rows).spacing(12).padding(24).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Status bar (bottom, full width)
// ---------------------------------------------------------------------------
fn status_bar(app: &BuzeeApp) -> El<'_> {
    let status = &app.state.status;
    let stat_status = app
        .state
        .statistics
        .as_ref()
        .map(|s| s.status.clone())
        .unwrap_or_default();
    let status_text = if status.is_empty() { stat_status } else { status.clone() };

    let left = row![
        text("●").size(10).class(TextKind::Purple),
        text(format!("{status_text}")).size(13).class(TextKind::Muted),
        text(format!("  •  {} files indexed", app.state.parsed_count))
            .size(13)
            .class(TextKind::Muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let right = row![
        button(text("Sync").size(13).class(TextKind::Default))
            .on_press(Message::ToggleSync)
            .class(ButtonKind::Ghost)
            .padding([8, 14]),
        button(text("OCR").size(13).class(TextKind::Default))
            .on_press(Message::StartOcr)
            .class(ButtonKind::Ghost)
            .padding([8, 14]),
    ]
    .spacing(6);

    container(
        row![left, Space::new().width(Length::Fill), right].align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(32.0))
    .class(ContainerKind::Panel)
    .padding([0, 24])
    .into()
}

// ---------------------------------------------------------------------------
// Sorting + formatting helpers
// ---------------------------------------------------------------------------
pub(crate) fn fmt_time(ts: i64) -> String {
    if ts <= 0 {
        return "—".to_string();
    }
    match Local.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%d/%m/%Y %H:%M").to_string(),
        _ => "—".to_string(),
    }
}

pub(crate) fn fmt_size(size: Option<f64>) -> String {
    match size {
        None => "—".to_string(),
        Some(s) if s <= 0.0 => "—".to_string(),
        Some(s) if s >= 1.0e9 => format!("{:.1} GB", s / 1.0e9),
        Some(s) if s >= 1.0e6 => format!("{:.1} MB", s / 1.0e6),
        Some(s) if s >= 1.0e3 => format!("{:.1} KB", s / 1.0e3),
        Some(s) => format!("{:.0} B", s),
    }
}

/// A three-digit-grouped number (thousands separator).
pub(crate) fn fmt_thousands(n: i64) -> String {
    let neg = n < 0;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}

fn date_presets() -> Vec<(String, i64, i64)> {
    use chrono::Duration;
    let now = Local::now();
    let mk = |days: i64, label: String| (label, (now - Duration::days(days)).timestamp(), now.timestamp());
    vec![
        mk(7, "Last 7 days".to_string()),
        mk(30, "Last 30 days".to_string()),
        mk(365, "Last year".to_string()),
    ]
}
