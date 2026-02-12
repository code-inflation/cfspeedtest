use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::Frame;

use crate::engine::types::TestType;
use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, data: &[f64], test_type: TestType) {
    if data.is_empty() || area.width < 10 || area.height < 4 {
        return;
    }

    let color = match test_type {
        TestType::Download => theme::DOWNLOAD_COLOR,
        TestType::Upload => theme::UPLOAD_COLOR,
    };

    let max_y = data.iter().cloned().fold(0.0_f64, f64::max).max(1.0);

    let points: Vec<(f64, f64)> = data
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(color))
        .data(&points);

    let x_axis = Axis::default()
        .bounds([0.0, (data.len().max(1) - 1) as f64])
        .style(Style::default().fg(theme::DIM_TEXT));

    let y_axis = Axis::default()
        .bounds([0.0, max_y * 1.1])
        .labels(vec![
            Span::from("0"),
            Span::from(format!("{:.0}", max_y / 2.0)),
            Span::from(format!("{:.0}", max_y)),
        ])
        .style(Style::default().fg(theme::DIM_TEXT));

    let chart = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);

    f.render_widget(chart, area);
}
