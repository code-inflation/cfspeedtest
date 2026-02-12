use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::app::{App, Phase};
use super::theme;
use super::widgets;

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Outer border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_COLOR));
    f.render_widget(block, size);

    let inner = Rect {
        x: size.x + 2,
        y: size.y + 1,
        width: size.width.saturating_sub(4),
        height: size.height.saturating_sub(2),
    };

    if inner.height < 3 || inner.width < 20 {
        return;
    }

    // Layout: header (1) | content (flex) | progress (1)
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(inner);

    // Header
    widgets::header::render(f, chunks[0], app);

    // Phase label
    render_phase_label(f, chunks[1], app);

    // Content area (depends on phase)
    let content = chunks[2];
    match app.phase {
        Phase::Connecting => {
            render_connecting(f, content);
        }
        Phase::Latency => {
            render_latency(f, content, app);
        }
        Phase::Download => {
            render_throughput(f, content, app);
        }
        Phase::Upload => {
            render_throughput(f, content, app);
        }
        Phase::Results => {
            widgets::results::render(f, content, app);
        }
    }

    // Progress bar
    widgets::progress::render(f, chunks[3], app);
}

fn render_phase_label(f: &mut Frame, area: Rect, app: &App) {
    let (label, color) = match app.phase {
        Phase::Connecting => ("CONNECTING", theme::DIM_TEXT),
        Phase::Latency => ("LATENCY", theme::LATENCY_COLOR),
        Phase::Download => ("DOWNLOAD", theme::DOWNLOAD_COLOR),
        Phase::Upload => ("UPLOAD", theme::UPLOAD_COLOR),
        Phase::Results => ("RESULTS", theme::BRIGHT_TEXT),
    };

    let line = Line::from(Span::styled(
        label,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(line), area);
}

fn render_connecting(f: &mut Frame, area: Rect) {
    let msg = Paragraph::new(Line::from(Span::styled(
        "Fetching server info...",
        Style::default().fg(theme::DIM_TEXT),
    )))
    .alignment(Alignment::Center);

    let centered = Rect {
        x: area.x,
        y: area.y + area.height / 2,
        width: area.width,
        height: 1,
    };
    f.render_widget(msg, centered);
}

fn render_latency(f: &mut Frame, area: Rect, app: &App) {
    let avg = app
        .latency_result
        .as_ref()
        .map(|r| r.avg_ms)
        .unwrap_or_else(|| {
            if app.latency_samples.is_empty() {
                0.0
            } else {
                app.latency_samples.iter().sum::<f64>() / app.latency_samples.len() as f64
            }
        });

    let min = app
        .latency_result
        .as_ref()
        .map(|r| r.min_ms)
        .unwrap_or_else(|| {
            app.latency_samples
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min)
        });
    let min = if min.is_infinite() { 0.0 } else { min };

    let max = app
        .latency_result
        .as_ref()
        .map(|r| r.max_ms)
        .unwrap_or_else(|| {
            app.latency_samples
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max)
        });
    let max = if max.is_infinite() { 0.0 } else { max };

    widgets::latency_plot::render(f, area, &app.latency_samples, avg, min, max);
}

fn render_throughput(f: &mut Frame, area: Rect, app: &App) {
    let test_type = app
        .current_test_type
        .unwrap_or(crate::engine::types::TestType::Download);

    if area.height < 6 {
        // Small terminal: just show speed gauge
        widgets::speed_gauge::render(f, area, app.current_mbps, test_type);
        return;
    }

    // Split: gauge (3) | chart (rest)
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(area);

    widgets::speed_gauge::render(f, chunks[0], app.current_mbps, test_type);
    widgets::live_chart::render(f, chunks[1], &app.chart_data, test_type);
}
