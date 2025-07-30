use crate::measurements::Measurement;
use crate::speedtest::{TestType, Metadata};
use crossbeam_channel::Receiver;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame, Terminal,
};
use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SpeedData {
    pub timestamp: Instant,
    pub speed: f64,
    pub test_type: TestType,
    pub payload_size: usize,
}

#[derive(Debug, Clone)]
pub struct LatencyData {
    pub timestamp: Instant,
    pub latency: f64,
}

#[derive(Debug, Clone)]
pub enum TestEvent {
    SpeedMeasurement(SpeedData),
    LatencyMeasurement(LatencyData),
    TestStarted(TestType, usize),
    TestCompleted(TestType, usize),
    AllTestsCompleted,
    MetadataReceived(Metadata),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct TestProgress {
    pub current_test: Option<TestType>,
    pub current_payload_size: Option<usize>,
    pub current_iteration: u32,
    pub total_iterations: u32,
    pub phase: TestPhase,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestPhase {
    Idle,
    Latency,
    Download,
    Upload,
    Completed,
}

pub struct DashboardState {
    pub download_speeds: VecDeque<SpeedData>,
    pub upload_speeds: VecDeque<SpeedData>,
    pub latency_measurements: VecDeque<LatencyData>,
    pub progress: TestProgress,
    pub current_download_speed: f64,
    pub current_upload_speed: f64,
    pub current_latency: f64,
    pub avg_latency: f64,
    pub min_latency: f64,
    pub max_latency: f64,
    pub measurements: Vec<Measurement>,
    pub start_time: Instant,
    pub max_data_points: usize,
    pub metadata: Option<Metadata>,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            download_speeds: VecDeque::new(),
            upload_speeds: VecDeque::new(),
            latency_measurements: VecDeque::new(),
            progress: TestProgress {
                current_test: None,
                current_payload_size: None,
                current_iteration: 0,
                total_iterations: 0,
                phase: TestPhase::Idle,
            },
            current_download_speed: 0.0,
            current_upload_speed: 0.0,
            current_latency: 0.0,
            avg_latency: 0.0,
            min_latency: f64::MAX,
            max_latency: 0.0,
            measurements: Vec::new(),
            start_time: Instant::now(),
            max_data_points: 100,
            metadata: None,
        }
    }
}

impl DashboardState {
    pub fn update(&mut self, event: TestEvent) {
        match event {
            TestEvent::SpeedMeasurement(data) => {
                match data.test_type {
                    TestType::Download => {
                        self.current_download_speed = data.speed;
                        self.download_speeds.push_back(data.clone());
                        if self.download_speeds.len() > self.max_data_points {
                            self.download_speeds.pop_front();
                        }
                    }
                    TestType::Upload => {
                        self.current_upload_speed = data.speed;
                        self.upload_speeds.push_back(data.clone());
                        if self.upload_speeds.len() > self.max_data_points {
                            self.upload_speeds.pop_front();
                        }
                    }
                }

                self.measurements.push(Measurement {
                    test_type: data.test_type,
                    payload_size: data.payload_size,
                    mbit: data.speed,
                });
            }
            TestEvent::LatencyMeasurement(data) => {
                self.current_latency = data.latency;
                self.latency_measurements.push_back(data.clone());
                if self.latency_measurements.len() > self.max_data_points {
                    self.latency_measurements.pop_front();
                }

                if data.latency < self.min_latency {
                    self.min_latency = data.latency;
                }
                if data.latency > self.max_latency {
                    self.max_latency = data.latency;
                }

                let sum: f64 = self.latency_measurements.iter().map(|l| l.latency).sum();
                self.avg_latency = sum / self.latency_measurements.len() as f64;
            }
            TestEvent::TestStarted(test_type, payload_size) => {
                self.progress.current_test = Some(test_type);
                self.progress.current_payload_size = Some(payload_size);
                self.progress.phase = match test_type {
                    TestType::Download => TestPhase::Download,
                    TestType::Upload => TestPhase::Upload,
                };
            }
            TestEvent::TestCompleted(_, _) => {
                self.progress.current_iteration += 1;
            }
            TestEvent::AllTestsCompleted => {
                self.progress.phase = TestPhase::Completed;
                self.progress.current_test = None;
                self.progress.current_payload_size = None;
            }
            TestEvent::MetadataReceived(metadata) => {
                self.metadata = Some(metadata);
            }
            TestEvent::Error(_) => {
                // Handle errors if needed
            }
        }
    }
}

pub struct App {
    pub state: DashboardState,
    pub should_quit: bool,
    pub event_receiver: Option<Receiver<TestEvent>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            state: DashboardState::default(),
            should_quit: false,
            event_receiver: None,
        }
    }

    pub fn with_receiver(mut self, receiver: Receiver<TestEvent>) -> Self {
        self.event_receiver = Some(receiver);
        self
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if self.should_quit {
                break;
            }

            if let Some(receiver) = &self.event_receiver {
                while let Ok(event) = receiver.try_recv() {
                    self.state.update(event);
                }
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.should_quit = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Progress bars
                Constraint::Min(10),   // Main content
                Constraint::Length(3), // Status
            ])
            .split(f.area());

        self.draw_title(f, chunks[0]);
        self.draw_progress_bars(f, chunks[1]);
        self.draw_main_content(f, chunks[2]);
        self.draw_status(f, chunks[3]);
    }

    fn draw_title(&self, f: &mut Frame, area: Rect) {
        let title_text = if let Some(ref metadata) = self.state.metadata {
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

    fn draw_progress_bars(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let download_progress = if self.state.progress.phase == TestPhase::Download {
            (self.state.progress.current_iteration as f64
                / self.state.progress.total_iterations as f64)
                .min(1.0)
        } else if matches!(
            self.state.progress.phase,
            TestPhase::Upload | TestPhase::Completed
        ) {
            1.0
        } else {
            0.0
        };

        let upload_progress = if self.state.progress.phase == TestPhase::Upload {
            (self.state.progress.current_iteration as f64
                / self.state.progress.total_iterations as f64)
                .min(1.0)
        } else if self.state.progress.phase == TestPhase::Completed {
            1.0
        } else {
            0.0
        };

        let download_gauge = Gauge::default()
            .block(Block::default().title("Download").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(download_progress);

        let upload_gauge = Gauge::default()
            .block(Block::default().title("Upload").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Blue))
            .ratio(upload_progress);

        f.render_widget(download_gauge, chunks[0]);
        f.render_widget(upload_gauge, chunks[1]);
    }

    fn draw_main_content(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        self.draw_speed_graphs(f, chunks[0]);
        self.draw_stats_and_boxplots(f, chunks[1]);
    }

    fn draw_speed_graphs(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.draw_download_graph(f, chunks[0]);
        self.draw_upload_graph(f, chunks[1]);
    }

    fn draw_download_graph(&self, f: &mut Frame, area: Rect) {
        let title = format!(
            "Download Speed ({:.1} Mbps)",
            self.state.current_download_speed
        );
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if !self.state.download_speeds.is_empty() {
            let graph_widget = crate::tui::widgets::LineGraph::new(&self.state.download_speeds)
                .color(Color::Green);
            f.render_widget(graph_widget, inner);
        }
    }

    fn draw_upload_graph(&self, f: &mut Frame, area: Rect) {
        let title = format!("Upload Speed ({:.1} Mbps)", self.state.current_upload_speed);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if !self.state.upload_speeds.is_empty() {
            let graph_widget =
                crate::tui::widgets::LineGraph::new(&self.state.upload_speeds).color(Color::Blue);
            f.render_widget(graph_widget, inner);
        }
    }

    fn draw_stats_and_boxplots(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.draw_latency_stats(f, chunks[0]);
        self.draw_boxplots(f, chunks[1]);
    }

    fn draw_latency_stats(&self, f: &mut Frame, area: Rect) {
        let stats_text = vec![
            Line::from(vec![
                Span::raw("Current: "),
                Span::styled(
                    format!("{:.1}ms", self.state.current_latency),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("Average: "),
                Span::styled(
                    format!("{:.1}ms", self.state.avg_latency),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::raw("Min/Max: "),
                Span::styled(
                    format!(
                        "{:.1}ms / {:.1}ms",
                        if self.state.min_latency == f64::MAX {
                            0.0
                        } else {
                            self.state.min_latency
                        },
                        self.state.max_latency
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

    fn draw_boxplots(&self, f: &mut Frame, area: Rect) {
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

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let status_text = match self.state.progress.phase {
            TestPhase::Idle => "Ready to start tests. Press 'q' to quit.".to_string(),
            TestPhase::Latency => "Running latency tests...".to_string(),
            TestPhase::Download => {
                if let (Some(payload_size), Some(_)) = (
                    self.state.progress.current_payload_size,
                    self.state.progress.current_test,
                ) {
                    format!(
                        "Testing Download {}MB [{}/{}]",
                        payload_size / 1_000_000,
                        self.state.progress.current_iteration,
                        self.state.progress.total_iterations
                    )
                } else {
                    "Testing Download...".to_string()
                }
            }
            TestPhase::Upload => {
                if let (Some(payload_size), Some(_)) = (
                    self.state.progress.current_payload_size,
                    self.state.progress.current_test,
                ) {
                    format!(
                        "Testing Upload {}MB [{}/{}]",
                        payload_size / 1_000_000,
                        self.state.progress.current_iteration,
                        self.state.progress.total_iterations
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
}
