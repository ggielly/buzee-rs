//! Real data charts rendered through plotters-iced2 (the Plotters backend for
//! iced 0.14). Everything is re-drawn each `view` call with the active app
//! palette, and the drawing surface is left transparent so a chart sits
//! seamlessly on top of its card background.

use crate::ui::message::Message;
use crate::ui::theme::{Palette, Theme};
use plotters::prelude::*;
use plotters::style::RGBColor;
use plotters_iced2::{Chart, ChartWidget};

/// One bar of the file-type distribution chart.
pub struct BarDatum {
    /// Short extension label, e.g. "PDF".
    pub label: String,
    /// Number of files of this type.
    pub count: u64,
    /// Brand color of the file type.
    pub color: RGBColor,
}

impl BarDatum {
    /// Create a bar from an iced color, converting it to the plotters palette.
    pub fn new(label: impl Into<String>, count: u64, color: iced::Color) -> Self {
        Self {
            label: label.into(),
            count,
            color: rgb(color),
        }
    }
}

/// Vertical bar chart of the "Files by Type" distribution.
///
/// The chart owns its data (no borrows) so it can be moved straight into a
/// [`ChartWidget`] and outlive the returned [`iced::Element`].
pub struct FileTypeChart {
    buckets: Vec<BarDatum>,
    text: RGBColor,
    axis: RGBColor,
}

impl FileTypeChart {
    /// Build a chart from raw buckets plus the active palette.
    pub fn new(buckets: Vec<BarDatum>, palette: &Palette) -> Self {
        Self {
            buckets,
            text: rgb(palette.foreground),
            axis: rgb(palette.muted_foreground),
        }
    }

    /// Render the chart as an iced element with the given fixed height.
    pub fn view(self, height: f32) -> iced::Element<'static, Message, Theme> {
        ChartWidget::new(self)
            .height(iced::Length::Fixed(height))
            .into()
    }
}

impl Chart<Message> for FileTypeChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(
        &self,
        _state: &Self::State,
        mut builder: ChartBuilder<DB>,
    ) {
        let n = self.buckets.len();
        if n == 0 {
            return;
        }

        let max_y = self
            .buckets
            .iter()
            .map(|b| b.count)
            .max()
            .unwrap_or(1)
            .max(1);

        let mut chart = builder
            .margin(6)
            .x_label_area_size(24)
            .y_label_area_size(32)
            .build_cartesian_2d((0u32..n as u32).into_segmented(), 0u64..(max_y + 1))
            .expect("invalid chart range");

        chart
            .configure_mesh()
            .disable_mesh()
            .disable_x_mesh()
            .axis_style(self.axis)
            .label_style(("sans-serif", 11.0).with_color(self.text))
            .y_labels(4)
            .x_label_formatter(&|x| match x {
                SegmentValue::CenterOf(v) | SegmentValue::Exact(v) => self
                    .buckets
                    .get(*v as usize)
                    .map(|b| b.label.clone())
                    .unwrap_or_default(),
                SegmentValue::Last => String::new(),
            })
            .draw()
            .expect("failed to draw the chart mesh");

        // One tiny series per bucket so each bar keeps its own brand color.
        for (i, bucket) in self.buckets.iter().enumerate() {
            chart
                .draw_series(
                    Histogram::vertical(&chart)
                        .style(bucket.color.filled())
                        .margin(8)
                        .data([(i as u32, bucket.count)]),
                )
                .expect("failed to draw a bar");
        }
    }
}

/// Convert an iced color (f32, 0..=1) to a plotters RGB color (u8).
fn rgb(c: iced::Color) -> RGBColor {
    RGBColor(
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
    )
}