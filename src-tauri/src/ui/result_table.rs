//! The virtualized results table built on `iced_table`.
//!
//! `iced_table` borrows its column descriptors and rows for the lifetime of the
//! returned element, so a column array cannot be a local. We therefore keep a
//! data-only snapshot (`ResultTableColumn`) inside [`BuzeeUiState`] that is
//! rebuilt whenever the sort, selection or results change, and render the
//! already-sorted `state.results` directly as rows.

use crate::infrastructure::database::models::DocumentSearchResult;
use crate::ui::icons::file_badge;
use crate::ui::message::Message;
use crate::ui::state::{BuzeeUiState, SortColumn};
use crate::ui::theme::{ButtonKind, ContainerKind, TextKind, Theme};
use crate::ui::BuzeeApp;
use iced::widget::{button, container, row, text};
use iced::{alignment, Alignment, Element, Length};
use iced_table::table;

/// Which logical column a [`ResultTableColumn`] describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultTableColumnKind {
    Type,
    Name,
    LastModified,
    LastOpened,
    Size,
    Location,
    Actions,
}

/// A data-only description of one result-table column. Live view data (theme,
/// sort, selected id) is copied in so the descriptor can live in app state and
/// be borrowed for the element's lifetime.
#[derive(Debug, Clone, Copy)]
pub struct ResultTableColumn {
    pub kind: ResultTableColumnKind,
    pub theme: Theme,
    pub sort: SortColumn,
    pub asc: bool,
    pub selected_id: Option<i32>,
}

/// Rebuild the column descriptors from the current state.
pub fn columns_for(state: &BuzeeUiState) -> Vec<ResultTableColumn> {
    use ResultTableColumnKind::*;
    let kinds = [
        Type, Name, LastModified, LastOpened, Size, Location, Actions,
    ];
    kinds
        .into_iter()
        .map(|kind| ResultTableColumn {
            kind,
            theme: state.theme,
            sort: state.sort.column,
            asc: state.sort.asc,
            selected_id: state.selected_result,
        })
        .collect()
}

/// The fixed width (in px) of a column.
fn column_width(kind: ResultTableColumnKind) -> f32 {
    use ResultTableColumnKind::*;
    match kind {
        Type => 64.0,
        Name => 300.0,
        LastModified => 150.0,
        LastOpened => 150.0,
        Size => 90.0,
        Location => 300.0,
        Actions => 118.0,
    }
}

/// Build the virtualized results table element.
pub fn view(app: &BuzeeApp) -> Element<'_, Message, Theme> {
    let pad: [f32; 2] = if app.state.compact_view { [6.0, 10.0] } else { [10.0, 10.0] };

    table::table(
        iced::widget::Id::new("results-table-header"),
        iced::widget::Id::new("results-table"),
        &app.state.result_columns,
        &app.state.results,
        |offset| Message::TableSync(offset),
    )
    .cell_padding(pad)
    .divider_width(1.0)
    .min_column_width(48.0)
    .into()
}

impl<'a> table::Column<'a, Message, Theme, iced::Renderer> for ResultTableColumn {
    type Row = DocumentSearchResult;

    fn header(
        &'a self,
        _col_index: usize,
    ) -> Element<'a, Message, Theme, iced::Renderer> {
        use ResultTableColumnKind::*;
        let sortable = matches!(
            self.kind,
            Name | LastModified | LastOpened | Size | Location | Type
        );

        if !sortable {
            return text("").size(12).class(TextKind::Muted).into();
        }

        let (label, column) = match self.kind {
            Name => ("Name", SortColumn::Name),
            Type => ("Type", SortColumn::Type),
            LastModified => ("Last Modified", SortColumn::LastModified),
            LastOpened => ("Last Opened", SortColumn::LastOpened),
            Size => ("Size", SortColumn::Size),
            Location => ("Location", SortColumn::Location),
            Actions => ("", SortColumn::Name),
        };

        let active = self.sort == column;
        let arrow = match (active, self.asc) {
            (true, true) => " ▲",
            (true, false) => " ▼",
            _ => "",
        };
        let color = if active { TextKind::Default } else { TextKind::Muted };

        button(text(format!("{label}{arrow}")).size(12).class(color))
            .on_press(Message::SortChanged(column))
            .class(ButtonKind::Ghost)
            .width(Length::Fill)
            .padding([8, 6])
            .into()
    }

    fn cell(
        &'a self,
        _col_index: usize,
        _row_index: usize,
        row: &'a DocumentSearchResult,
    ) -> Element<'a, Message, Theme, iced::Renderer> {
        use ResultTableColumnKind::*;
        let selected = self.selected_id == Some(row.id);

        let base: Element<'a, Message, Theme, iced::Renderer> = match self.kind {
            Type => container(file_badge(&row.file_type, &self.theme))
                .width(Length::Fixed(32.0))
                .align_x(alignment::Horizontal::Center)
                .into(),
            Name => text(if row.name.is_empty() { &row.path } else { &row.name })
                .size(14)
                .class(TextKind::Default)
                .into(),
            LastModified => text(crate::ui::view::fmt_time(row.last_modified))
                .size(13)
                .class(TextKind::Muted)
                .into(),
            LastOpened => text(crate::ui::view::fmt_time(row.last_opened))
                .size(13)
                .class(TextKind::Muted)
                .into(),
            Size => text(crate::ui::view::fmt_size(row.size))
                .size(13)
                .class(TextKind::Muted)
                .into(),
            Location => text(parent_dir(&row.path)).size(13).class(TextKind::Muted).into(),
            Actions => row![
                button(text("Open").size(12).class(TextKind::Purple))
                    .on_press(Message::OpenResult(row.path.clone()))
                    .class(ButtonKind::Ghost)
                    .padding([4, 8]),
                button(text("Reveal").size(12).class(TextKind::Muted))
                    .on_press(Message::RevealResult(row.path.clone()))
                    .class(ButtonKind::Ghost)
                    .padding([4, 8]),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .into(),
        };

        // The whole row is clickable to select (except the actions column).
        let cell_el: Element<'a, Message, Theme, iced::Renderer> = if self.kind == Actions {
            base
        } else {
            button(base)
                .on_press(Message::SelectResult(row.id))
                .class(ButtonKind::Ghost)
                .width(Length::Fill)
                .into()
        };

        container(cell_el)
            .width(Length::Fill)
            .class(if selected && self.kind != Actions {
                ContainerKind::Selected
            } else {
                ContainerKind::Transparent
            })
            .into()
    }

    fn width(&self) -> f32 {
        column_width(self.kind)
    }

    fn resize_offset(&self) -> Option<f32> {
        None
    }
}

pub(crate) fn parent_dir(path: &str) -> String {
    match path.rsplit_once(['/', '\\']) {
        Some((dir, _)) => dir.to_string(),
        None => path.to_string(),
    }
}