//! Dashboard screen: KPI cards, scan status, files-by-type bar + legend,
//! largest files and recently modified lists. Data comes from the
//! `DashboardStats` snapshot requested on app start and on refresh.

use crate::domain::types::DashboardStats;
use crate::infrastructure::database::models::DocumentSearchResult;
use crate::ui::charts::{BarDatum, FileTypeChart};
use crate::ui::icons::{file_badge, logo_mark};
use crate::ui::message::Message;
use crate::ui::state::Screen;
use crate::ui::theme::{ButtonKind, ContainerKind, TextKind, Theme};
use crate::ui::view::{fmt_size, fmt_thousands, fmt_time};
use crate::ui::BuzeeApp;
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{alignment, Alignment, Element, Length};

/// A brand-themed element.
type El<'a> = Element<'a, Message, Theme>;

pub fn view(app: &BuzeeApp) -> El<'_> {
    let stats: Option<&DashboardStats> = app.state.dashboard.as_ref();

    column![
        heading(),
        scrollable(
            column![
                kpi_cards(stats, app),
                scan_status_row(app, stats),
                row![files_by_type(stats, app), side_lists(stats, app)]
                    .spacing(20)
                    .align_y(Alignment::Start),
            ]
            .spacing(20)
            .padding([24, 32])
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .width(Length::FillPortion(4))
    .height(Length::Fill)
    .into()
}

fn heading<'a>() -> El<'a> {
    container(
        row![
            column![
                text("Dashboard").size(24).class(TextKind::Default),
                text("An overview of your index").size(13).class(TextKind::Muted),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            button(text("Refresh").size(13).class(TextKind::Purple))
                .on_press(Message::RefreshDashboard)
                .class(ButtonKind::Purple)
                .padding([8, 16]),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .into()
}

fn kpi_cards<'a>(stats: Option<&'a DashboardStats>, app: &'a BuzeeApp) -> El<'a> {
    let s = |f: fn(&DashboardStats) -> i64| stats.map(f).unwrap_or(0);
    let sz = |f: fn(&DashboardStats) -> f64| stats.map(f).unwrap_or(0.0);
    let cards = [
        ("Files", fmt_thousands(s(|x| x.total_files)), app.state.theme.palette().purple),
        ("Folders", fmt_thousands(s(|x| x.total_folders)), app.state.theme.palette().light_purple),
        ("Indexed", fmt_thousands(s(|x| x.parsed_files)), app.state.theme.palette().success),
        ("Unindexed", fmt_thousands(s(|x| x.unparsed_files)), app.state.theme.palette().hot_pink),
        ("Total size", fmt_size(Some(sz(|x| x.total_size_bytes))), app.state.theme.palette().lime_green),
        ("Avg size", fmt_size(Some(sz(|x| x.average_size_bytes))), app.state.theme.palette().muted_foreground),
    ];

    let mut row_els: Vec<El<'a>> = Vec::new();
    for (label, value, color) in cards {
        row_els.push(
            container(
                column![
                    container(text("▮").size(18).class(TextKind::Color(color)))
                        .align_x(alignment::Horizontal::Left),
                    text(value).size(22).class(TextKind::Default),
                    text(label).size(12).class(TextKind::Muted),
                ]
                .spacing(6),
            )
            .padding(16)
            .class(ContainerKind::Card)
            .width(Length::FillPortion(1))
            .into(),
        );
    }
    row(row_els).spacing(14).width(Length::Fill).into()
}

fn scan_status_row<'a>(app: &'a BuzeeApp, stats: Option<&'a DashboardStats>) -> El<'a> {
    let running = stats.map(|x| x.scan_running).unwrap_or(false);
    let auto_sync = stats.map(|x| x.auto_sync_enabled).unwrap_or(false);
    let last_scan = stats.map(|x| x.last_scan_time).unwrap_or(0);
    let next_scan = stats.map(|x| x.next_scan_in_seconds).unwrap_or(-1);

    let dot_color = if running { app.state.theme.palette().hot_pink } else { app.state.theme.palette().success };
    let status = if running {
        "Scanning…"
    } else if auto_sync {
        "Auto sync enabled"
    } else {
        "Sync paused"
    };

    let last_scan = if last_scan > 0 {
        format!("Last scan: {}", fmt_time(last_scan))
    } else {
        "Last scan: —".to_string()
    };
    let next_scan = if next_scan >= 0 {
        format!("Next scan in ~{}s", next_scan)
    } else {
        "Next scan: —".to_string()
    };

    container(
        row![
            text("●").size(10).class(TextKind::Color(dot_color)),
            text(status).size(13).class(TextKind::Default),
            text(format!("  •  {last_scan}  •  {next_scan}")).size(13).class(TextKind::Muted),
            Space::new().width(Length::Fill),
            button(text("Search").size(13).class(TextKind::Purple))
                .on_press(Message::Navigate(Screen::Search))
                .class(ButtonKind::Ghost)
                .padding([8, 16]),
        ]
        .align_y(Alignment::Center),
    )
    .padding([14, 18])
    .class(ContainerKind::Panel)
    .width(Length::Fill)
    .into()
}

fn files_by_type<'a>(stats: Option<&'a DashboardStats>, app: &'a BuzeeApp) -> El<'a> {
    let buckets = stats.map(|x| &x.filetype_counts).cloned().unwrap_or_default();
    let total: i64 = buckets.iter().map(|b| b.count).sum();
    let max_count = buckets.iter().map(|b| b.count).max().unwrap_or(0);

    // Top-N file types are drawn as a real bar chart; the rest fold into "Other".
    let mut chart_data: Vec<BarDatum> = Vec::new();
    let mut top: Vec<(String, i64)> = Vec::new();
    let mut other: i64 = 0;
    let mut sorted: Vec<&crate::domain::types::DashboardBuckets> =
        buckets.iter().filter(|b| b.count > 0).collect();
    sorted.sort_by(|a, b| b.count.cmp(&a.count));
    for b in sorted {
        if top.len() < 8 {
            top.push((b.file_type.clone(), b.count));
        } else {
            other += b.count;
        }
    }
    for (label, count) in top {
        let color = app.state.theme.palette().filetype_color(&label);
        chart_data.push(BarDatum::new(label.to_uppercase(), count as u64, color));
    }
    if other > 0 {
        let color = app.state.theme.palette().muted_foreground;
        chart_data.push(BarDatum::new("Other".to_string(), other as u64, color));
    }
    let chart: El<'a> = FileTypeChart::new(chart_data, &app.state.theme.palette()).view(130.0);

    let legend: Vec<El<'a>> = buckets
        .iter()
        .take(12)
        .map(|b| {
            let color = app.state.theme.palette().filetype_color(&b.file_type);
            let share = if total > 0 {
                (b.count as f64 * 100.0 / total as f64) as i64
            } else {
                0
            };
            row![
                container(container(text("").size(1)).width(Length::Fixed(10.0)).height(Length::Fixed(10.0)))
                    .width(Length::Fixed(10.0))
                    .class(ContainerKind::Fill(color)),
                text(format!("{}  {}  ({share}%)", b.file_type.to_uppercase(), fmt_thousands(b.count)))
                    .size(12)
                    .class(TextKind::Muted),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into()
        })
        .collect();

    let percentage = if total > 0 { ((stats.map(|x| x.parsed_files).unwrap_or(0) as f64 / total as f64) * 100.0) as i64 } else { 0 };
    let max_files = fmt_thousands(max_count);

    container(
        column![
            text("Files by Type").size(16).class(TextKind::Default),
            text(format!("Showing {total} files  •  most common type appears {max_files} times"))
                .size(12)
                .class(TextKind::Muted),
            column![
                chart,
                column(legend).spacing(8),
            ]
            .spacing(14),
            container(
                row![
                    column![
                        text("Parsing Progress").size(12).class(TextKind::Muted),
                        text(format!("{}%", percentage)).size(16).class(TextKind::Default),
                    ]
                    .spacing(2),
                    Space::new().width(Length::Fill),
                    text(format!(
                        "{} / {} parsed",
                        fmt_thousands(stats.map(|x| x.parsed_files).unwrap_or(0)),
                        fmt_thousands(total)
                    ))
                    .size(12)
                    .class(TextKind::Muted),
                ]
                .align_y(Alignment::Center),
            )
            .padding([12, 0]),
        ]
        .spacing(14),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::FillPortion(3))
    .into()
}

fn side_lists<'a>(stats: Option<&'a DashboardStats>, app: &'a BuzeeApp) -> El<'a> {
    column![
        file_list("Largest Files", stats.map(|x| x.top_largest.as_slice()), app),
        file_list("Recently Modified", stats.map(|x| x.top_recent.as_slice()), app),
    ]
    .spacing(20)
    .width(Length::FillPortion(2))
    .into()
}

fn file_list<'a>(title: &'a str, files: Option<&'a [DocumentSearchResult]>, app: &'a BuzeeApp) -> El<'a> {
    let files = files.unwrap_or(&[]);
    let mut rows: Vec<El<'a>> = Vec::new();
    for res in files.iter().take(6) {
        let label = if res.name.is_empty() { &res.path } else { &res.name };
        rows.push(
            container(
                row![
                    file_badge(&res.file_type, &app.state.theme),
                    column![
                        text(label).size(13).class(TextKind::Default).width(Length::Fill),
                        text(fmt_time(res.last_modified)).size(11).class(TextKind::Muted),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    text(fmt_size(res.size)).size(12).class(TextKind::Muted),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([8, 0])
            .width(Length::Fill)
            .into(),
        );
    }

    if rows.is_empty() {
        rows.push(
            container(
                column![
                    logo_mark(40.0),
                    text("No files indexed yet").size(13).class(TextKind::Muted),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(20)
            .align_x(alignment::Horizontal::Center)
            .into(),
        );
    }

    container(
        column![text(title).size(16).class(TextKind::Default), column(rows).spacing(2)]
            .spacing(10),
    )
    .padding(20)
    .class(ContainerKind::Card)
    .width(Length::Fill)
    .into()
}
