use crate::tui::app::{DashboardState, TestPhase};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

pub fn render_dashboard(f: &mut Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Progress bars
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status
        ])
        .split(f.area());

    render_title(f, chunks[0], state);
    render_progress_bars(f, chunks[1], state);
    render_main_content(f, chunks[2], state);
    render_status(f, chunks[3], state);
}

fn render_title(f: &mut Frame, area: Rect, state: &DashboardState) {
    let title_text = if let Some(ref metadata) = state.metadata {
        format!(
            "Cloudflare Speed Test - {} {} | IP: {} | Colo: {}",
            metadata.city, metadata.country, metadata.ip, metadata.colo
        )
    } else {
        "Cloudflare Speed Test - Loading...".to_string()
    };

    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, area);
}

fn render_progress_bars(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let download_progress = match state.progress.phase {
        TestPhase::Download => {
            if state.progress.total_iterations > 0 {
                state.progress.current_iteration as f64 / state.progress.total_iterations as f64
            } else {
                0.0
            }
        }
        TestPhase::Upload | TestPhase::Completed => 1.0,
        _ => 0.0,
    };

    let upload_progress = match state.progress.phase {
        TestPhase::Upload => {
            if state.progress.total_iterations > 0 {
                state.progress.current_iteration as f64 / state.progress.total_iterations as f64
            } else {
                0.0
            }
        }
        TestPhase::Completed => 1.0,
        _ => 0.0,
    };

    let download_gauge = Gauge::default()
        .block(Block::default().title("Download").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(download_progress.min(1.0));

    let upload_gauge = Gauge::default()
        .block(Block::default().title("Upload").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Blue))
        .ratio(upload_progress.min(1.0));

    f.render_widget(download_gauge, chunks[0]);
    f.render_widget(upload_gauge, chunks[1]);
}

fn render_main_content(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_speed_graphs(f, chunks[0], state);
    render_stats_and_boxplots(f, chunks[1], state);
}

fn render_speed_graphs(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_download_graph(f, chunks[0], state);
    render_upload_graph(f, chunks[1], state);
}

fn render_download_graph(f: &mut Frame, area: Rect, state: &DashboardState) {
    let title = format!("Download Speed ({:.1} Mbps)", state.current_download_speed);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !state.download_speeds.is_empty() {
        let speeds: Vec<f64> = state.download_speeds.iter().map(|d| d.speed).collect();
        let graph_widget = crate::tui::widgets::SimpleLineChart::new(&speeds).color(Color::Green);
        f.render_widget(graph_widget, inner);
    } else {
        let placeholder =
            Paragraph::new("Waiting for data...").style(Style::default().fg(Color::Gray));
        f.render_widget(placeholder, inner);
    }
}

fn render_upload_graph(f: &mut Frame, area: Rect, state: &DashboardState) {
    let title = format!("Upload Speed ({:.1} Mbps)", state.current_upload_speed);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !state.upload_speeds.is_empty() {
        let speeds: Vec<f64> = state.upload_speeds.iter().map(|d| d.speed).collect();
        let graph_widget = crate::tui::widgets::SimpleLineChart::new(&speeds).color(Color::Blue);
        f.render_widget(graph_widget, inner);
    } else {
        let placeholder =
            Paragraph::new("Waiting for data...").style(Style::default().fg(Color::Gray));
        f.render_widget(placeholder, inner);
    }
}

fn render_stats_and_boxplots(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_latency_stats(f, chunks[0], state);
    render_boxplots(f, chunks[1], state);
}

fn render_latency_stats(f: &mut Frame, area: Rect, state: &DashboardState) {
    let stats_text = vec![
        Line::from(vec![
            Span::raw("Current: "),
            Span::styled(
                format!("{:.1}ms", state.current_latency),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Average: "),
            Span::styled(
                format!("{:.1}ms", state.avg_latency),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("Min/Max: "),
            Span::styled(
                format!(
                    "{:.1}ms / {:.1}ms",
                    if state.min_latency == f64::MAX {
                        0.0
                    } else {
                        state.min_latency
                    },
                    state.max_latency
                ),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(stats_text).block(
        Block::default()
            .title("Latency Stats")
            .borders(Borders::ALL),
    );
    f.render_widget(paragraph, area);
}

fn render_boxplots(f: &mut Frame, area: Rect, _state: &DashboardState) {
    let boxplot_text = vec![
        Line::from("Download: |----[===:===]----|"),
        Line::from("Upload:   |--[=====:=====]--|"),
        Line::from("Latency:  |-----[=:=]-------|"),
    ];

    let paragraph = Paragraph::new(boxplot_text).block(
        Block::default()
            .title("Measurement Boxplots")
            .borders(Borders::ALL),
    );
    f.render_widget(paragraph, area);
}

fn render_status(f: &mut Frame, area: Rect, state: &DashboardState) {
    let status_text = match state.progress.phase {
        TestPhase::Idle => "Ready to start tests. Press 'q' to quit.".to_string(),
        TestPhase::Latency => "Running latency tests...".to_string(),
        TestPhase::Download => {
            if let Some(payload_size) = state.progress.current_payload_size {
                format!(
                    "Testing Download {}MB [{}/{}]",
                    payload_size / 1_000_000,
                    state.progress.current_iteration,
                    state.progress.total_iterations
                )
            } else {
                "Testing Download...".to_string()
            }
        }
        TestPhase::Upload => {
            if let Some(payload_size) = state.progress.current_payload_size {
                format!(
                    "Testing Upload {}MB [{}/{}]",
                    payload_size / 1_000_000,
                    state.progress.current_iteration,
                    state.progress.total_iterations
                )
            } else {
                "Testing Upload...".to_string()
            }
        }
        TestPhase::Completed => "All tests completed! Press 'q' to quit.".to_string(),
    };

    let paragraph = Paragraph::new(status_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(paragraph, area);
}
