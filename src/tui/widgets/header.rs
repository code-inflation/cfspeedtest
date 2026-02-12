use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let title = Span::styled(
        " CLOUDFLARE SPEED TEST ",
        Style::default()
            .fg(theme::HEADER_COLOR)
            .add_modifier(Modifier::BOLD),
    );

    let info = if let Some(ref meta) = app.metadata {
        Span::styled(
            format!(" {} · {} · {} ", meta.colo, meta.country, meta.ip),
            Style::default().fg(theme::DIM_TEXT),
        )
    } else {
        Span::styled(" Connecting... ", Style::default().fg(theme::DIM_TEXT))
    };

    let line = Line::from(vec![title, Span::raw("  "), info]);
    let paragraph = Paragraph::new(line);
    f.render_widget(paragraph, area);
}
