use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::types::TestType;
use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, mbps: f64, test_type: TestType) {
    let color = match test_type {
        TestType::Download => theme::DOWNLOAD_COLOR,
        TestType::Upload => theme::UPLOAD_COLOR,
    };

    let speed_text = format!("{:.1}", mbps);
    let lines = vec![
        Line::from(Span::styled(
            speed_text,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("Mbps", Style::default().fg(theme::DIM_TEXT))),
    ];

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}
