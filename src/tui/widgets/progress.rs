use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let progress = app.overall_progress();
    let bar_width = area.width.saturating_sub(20) as usize;
    let filled = (progress * bar_width as f64) as usize;
    let empty = bar_width.saturating_sub(filled);

    let label = match &app.phase {
        crate::tui::app::Phase::Connecting => "Connecting...".to_string(),
        crate::tui::app::Phase::Latency => {
            format!("Latency {}/{}", app.latency_index, app.latency_total)
        }
        crate::tui::app::Phase::Download | crate::tui::app::Phase::Upload => {
            let ps = app
                .current_payload_size
                .map(|p| p.to_string())
                .unwrap_or_default();
            format!(
                "{} {} · {}/{}",
                app.current_test_type
                    .map(|t| t.to_string())
                    .unwrap_or_default(),
                ps,
                app.throughput_index,
                app.throughput_total
            )
        }
        crate::tui::app::Phase::Results => "Complete".to_string(),
    };

    let line = Line::from(vec![
        Span::styled("▰".repeat(filled), Style::default().fg(theme::HEADER_COLOR)),
        Span::styled("▱".repeat(empty), Style::default().fg(theme::DIM_TEXT)),
        Span::raw("  "),
        Span::styled(label, Style::default().fg(theme::DIM_TEXT)),
    ]);

    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}
