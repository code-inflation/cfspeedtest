use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, samples: &[f64], avg: f64, min: f64, max: f64) {
    if area.height < 4 {
        return;
    }

    // Sparkline for latency samples
    let spark_height = area.height.saturating_sub(2).min(3);
    let spark_area = Rect {
        x: area.x + 2,
        y: area.y,
        width: area.width.saturating_sub(4),
        height: spark_height,
    };

    if !samples.is_empty() {
        // Scale samples to u64 range for sparkline (multiply by 100 for precision)
        let spark_data: Vec<u64> = samples.iter().map(|&v| (v * 100.0) as u64).collect();
        let sparkline = Sparkline::default()
            .data(&spark_data)
            .style(Style::default().fg(theme::LATENCY_COLOR));
        f.render_widget(sparkline, spark_area);
    }

    // Stats line below
    let stats_area = Rect {
        x: area.x,
        y: area.y + spark_height + 1,
        width: area.width,
        height: 1,
    };

    let stats_line = Line::from(vec![
        Span::styled(
            format!("{avg:.1} ms"),
            Style::default().fg(theme::LATENCY_COLOR),
        ),
        Span::styled(" avg    ", Style::default().fg(theme::DIM_TEXT)),
        Span::styled(format!("{min:.1}"), Style::default().fg(theme::DIM_TEXT)),
        Span::styled(" min    ", Style::default().fg(theme::DIM_TEXT)),
        Span::styled(format!("{max:.1}"), Style::default().fg(theme::DIM_TEXT)),
        Span::styled(" max", Style::default().fg(theme::DIM_TEXT)),
    ]);

    let paragraph = Paragraph::new(stats_line).alignment(Alignment::Center);
    f.render_widget(paragraph, stats_area);
}
