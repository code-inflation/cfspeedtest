use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::engine::types::PayloadStats;
use crate::tui::app::App;
use crate::tui::theme;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 6 {
        return;
    }

    let mut y = area.y;

    // Hero numbers
    let hero_area = Rect {
        x: area.x,
        y,
        width: area.width,
        height: 3,
    };
    render_hero(f, hero_area, app);
    y += 4;

    // Stats tables
    if let Some(ref dl) = app.download_result {
        let table_height = (dl.stats.len() as u16 + 2).min(area.height.saturating_sub(y - area.y));
        let table_area = Rect {
            x: area.x + 1,
            y,
            width: area.width.saturating_sub(2),
            height: table_height,
        };
        render_stats_table(f, table_area, "Download", theme::DOWNLOAD_COLOR, &dl.stats);
        y += table_height + 1;
    }

    if let Some(ref ul) = app.upload_result {
        let remaining = area.height.saturating_sub(y - area.y);
        if remaining >= 3 {
            let table_height = (ul.stats.len() as u16 + 2).min(remaining);
            let table_area = Rect {
                x: area.x + 1,
                y,
                width: area.width.saturating_sub(2),
                height: table_height,
            };
            render_stats_table(f, table_area, "Upload", theme::UPLOAD_COLOR, &ul.stats);
            y += table_height + 1;
        }
    }

    // Quit hint
    let remaining = area.height.saturating_sub(y - area.y);
    if remaining >= 1 {
        let quit_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        let quit = Paragraph::new(Line::from(Span::styled(
            "Press q to quit",
            Style::default().fg(theme::DIM_TEXT),
        )))
        .alignment(Alignment::Center);
        f.render_widget(quit, quit_area);
    }
}

fn render_hero(f: &mut Frame, area: Rect, app: &App) {
    let mut spans_top = Vec::new();
    let mut spans_mid = Vec::new();

    let col_width = area.width / 3;

    if let Some(ref dl) = app.download_result {
        spans_top.push(Span::styled(
            pad_center("↓ DOWNLOAD", col_width as usize),
            Style::default().fg(theme::DOWNLOAD_COLOR),
        ));
        spans_mid.push(Span::styled(
            pad_center(&format!("{:.1} Mbps", dl.overall_mbps), col_width as usize),
            Style::default()
                .fg(theme::DOWNLOAD_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans_top.push(Span::raw(pad_center("", col_width as usize)));
        spans_mid.push(Span::raw(pad_center("", col_width as usize)));
    }

    if let Some(ref ul) = app.upload_result {
        spans_top.push(Span::styled(
            pad_center("↑ UPLOAD", col_width as usize),
            Style::default().fg(theme::UPLOAD_COLOR),
        ));
        spans_mid.push(Span::styled(
            pad_center(&format!("{:.1} Mbps", ul.overall_mbps), col_width as usize),
            Style::default()
                .fg(theme::UPLOAD_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans_top.push(Span::raw(pad_center("", col_width as usize)));
        spans_mid.push(Span::raw(pad_center("", col_width as usize)));
    }

    if let Some(ref lat) = app.latency_result {
        spans_top.push(Span::styled(
            pad_center("⏱ LATENCY", col_width as usize),
            Style::default().fg(theme::LATENCY_COLOR),
        ));
        spans_mid.push(Span::styled(
            pad_center(&format!("{:.1} ms", lat.avg_ms), col_width as usize),
            Style::default()
                .fg(theme::LATENCY_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let lines = vec![
        Line::from(spans_top),
        Line::from(vec![]),
        Line::from(spans_mid),
    ];
    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}

fn render_stats_table(
    f: &mut Frame,
    area: Rect,
    title: &str,
    color: ratatui::style::Color,
    stats: &[PayloadStats],
) {
    let header_cells = [
        Cell::from(Span::styled(
            title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled("avg", Style::default().fg(theme::DIM_TEXT))),
        Cell::from(Span::styled("min", Style::default().fg(theme::DIM_TEXT))),
        Cell::from(Span::styled("max", Style::default().fg(theme::DIM_TEXT))),
        Cell::from(Span::raw("")),
    ];
    let header = Row::new(header_cells);

    let rows: Vec<Row> = stats
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("{:>6}", s.payload_size),
                    Style::default().fg(theme::DIM_TEXT),
                )),
                Cell::from(Span::styled(
                    format!("{:>7.1}", s.avg),
                    Style::default().fg(theme::BRIGHT_TEXT),
                )),
                Cell::from(Span::styled(
                    format!("{:>7.1}", s.min),
                    Style::default().fg(theme::DIM_TEXT),
                )),
                Cell::from(Span::styled(
                    format!("{:>7.1}", s.max),
                    Style::default().fg(theme::DIM_TEXT),
                )),
                Cell::from(Span::styled("Mbps", Style::default().fg(theme::DIM_TEXT))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Length(5),
    ];

    let table = Table::new(rows, widths).header(header);
    f.render_widget(table, area);
}

fn pad_center(s: &str, width: usize) -> String {
    if s.len() >= width {
        return s.to_string();
    }
    let pad = (width - s.len()) / 2;
    format!("{:>width$}", s, width = s.len() + pad)
}
