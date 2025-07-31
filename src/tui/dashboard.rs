use crate::tui::app::{DashboardState, TestPhase};
use crate::tui::theme::{ThemedStyles, TokyoNight};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_dashboard(f: &mut Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(4), // Progress bars + Latency stats
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status
        ])
        .split(f.area());

    render_title(f, chunks[0], state);
    render_progress_and_latency(f, chunks[1], state);
    render_main_content(f, chunks[2], state);
    render_status(f, chunks[3], state);
}

fn render_title(f: &mut Frame, area: Rect, state: &DashboardState) {
    let title_text = if let Some(ref metadata) = state.metadata {
        format!(
            " Cloudflare Speed Test - {} {} | IP: {} | Colo: {}",
            metadata.city, metadata.country, metadata.ip, metadata.colo
        )
    } else {
        " Cloudflare Speed Test - Loading...".to_string()
    };

    let title = Paragraph::new(title_text)
        .style(ThemedStyles::title())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(ThemedStyles::title_border()),
        );
    f.render_widget(title, area);
}

fn render_progress_and_latency(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Download status
            Constraint::Length(1), // Upload status
            Constraint::Length(1), // Latency stats
        ])
        .split(area);

    // Download status
    let download_style = match state.progress.phase {
        TestPhase::Download => ThemedStyles::progress_download_active(),
        TestPhase::Completed => ThemedStyles::progress_download_complete(),
        _ if state.progress.download_status.contains("Completed") => {
            ThemedStyles::progress_download_complete()
        }
        _ => ThemedStyles::progress_inactive(),
    };

    let download_text = format!(" Download: {}", state.progress.download_status);
    let download_paragraph = Paragraph::new(download_text).style(download_style);
    f.render_widget(download_paragraph, chunks[0]);

    // Upload status
    let upload_style = match state.progress.phase {
        TestPhase::Upload => ThemedStyles::progress_upload_active(),
        TestPhase::Completed => ThemedStyles::progress_upload_complete(),
        _ if state.progress.upload_status.contains("Completed") => {
            ThemedStyles::progress_upload_complete()
        }
        _ => ThemedStyles::progress_inactive(),
    };

    let upload_text = format!(" Upload:   {}", state.progress.upload_status);
    let upload_paragraph = Paragraph::new(upload_text).style(upload_style);
    f.render_widget(upload_paragraph, chunks[1]);

    // Latency stats in a compact single line
    let latency_text = format!(
        " Latency:  Current: {:.1}ms | Average: {:.1}ms | Min/Max: {:.1}ms / {:.1}ms",
        state.current_latency,
        state.avg_latency,
        if state.min_latency == f64::MAX {
            0.0
        } else {
            state.min_latency
        },
        state.max_latency
    );
    let latency_paragraph = Paragraph::new(latency_text).style(ThemedStyles::latency_stats());
    f.render_widget(latency_paragraph, chunks[2]);
}

fn render_main_content(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_speed_graphs(f, chunks[0], state);
    render_boxplots(f, chunks[1], state);
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
        .border_style(ThemedStyles::download_graph_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !state.download_speeds.is_empty() {
        let speeds: Vec<f64> = state.download_speeds.iter().map(|d| d.speed).collect();
        let graph_widget =
            crate::tui::widgets::SimpleLineChart::new(&speeds).color(TokyoNight::DOWNLOAD_PRIMARY);
        f.render_widget(graph_widget, inner);
    } else {
        let placeholder =
            Paragraph::new("Waiting for data...").style(ThemedStyles::graph_placeholder());
        f.render_widget(placeholder, inner);
    }
}

fn render_upload_graph(f: &mut Frame, area: Rect, state: &DashboardState) {
    let title = format!("Upload Speed ({:.1} Mbps)", state.current_upload_speed);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(ThemedStyles::upload_graph_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    if !state.upload_speeds.is_empty() {
        let speeds: Vec<f64> = state.upload_speeds.iter().map(|d| d.speed).collect();
        let graph_widget =
            crate::tui::widgets::SimpleLineChart::new(&speeds).color(TokyoNight::UPLOAD_PRIMARY);
        f.render_widget(graph_widget, inner);
    } else {
        let placeholder =
            Paragraph::new("Waiting for data...").style(ThemedStyles::graph_placeholder());
        f.render_widget(placeholder, inner);
    }
}

fn render_boxplots(f: &mut Frame, area: Rect, state: &DashboardState) {
    let boxplot_grid = crate::tui::widgets::BoxplotGrid::new(&state.measurements);
    f.render_widget(boxplot_grid, area);
}

fn render_status(f: &mut Frame, area: Rect, state: &DashboardState) {
    let (status_text, status_style) = match state.progress.phase {
        TestPhase::Idle => (
            " Ready to start tests. Press 'q' to quit.".to_string(),
            ThemedStyles::status_idle(),
        ),
        TestPhase::Latency => (
            " Running latency tests...".to_string(),
            ThemedStyles::status_active(),
        ),
        TestPhase::Download => {
            if let Some(payload_size) = state.progress.current_payload_size {
                (
                    format!(
                        " Testing Download {}MB [{}/{}]",
                        payload_size / 1_000_000,
                        state.progress.current_iteration,
                        state.progress.total_iterations
                    ),
                    ThemedStyles::status_active(),
                )
            } else {
                (
                    " Testing Download...".to_string(),
                    ThemedStyles::status_active(),
                )
            }
        }
        TestPhase::Upload => {
            if let Some(payload_size) = state.progress.current_payload_size {
                (
                    format!(
                        " Testing Upload {}MB [{}/{}]",
                        payload_size / 1_000_000,
                        state.progress.current_iteration,
                        state.progress.total_iterations
                    ),
                    ThemedStyles::status_active(),
                )
            } else {
                (
                    " Testing Upload...".to_string(),
                    ThemedStyles::status_active(),
                )
            }
        }
        TestPhase::Completed => (
            " All tests completed! Press 'q' to quit.".to_string(),
            ThemedStyles::status_complete(),
        ),
    };

    let paragraph = Paragraph::new(status_text).style(status_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(ThemedStyles::status_border()),
    );
    f.render_widget(paragraph, area);
}
