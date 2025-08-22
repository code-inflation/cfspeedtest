use crate::measurements::Measurement;
use crate::speedtest::{Metadata, TestType};
use crate::tui::theme::{ThemedStyles, TokyoNight};
use crossbeam_channel::Receiver;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
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
    TestPhaseStarted(TestType, u32, Vec<usize>), // test_type, nr_tests, payload_sizes
    PayloadSizeStarted(TestType, usize, usize),  // test_type, payload_size, payload_index
    PayloadSizeCompleted(TestType, usize),
    TestPhaseCompleted(TestType, f64), // test_type, average_speed
    TestsSkipped(TestType, String),    // test_type, reason
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
    pub download_completed_tests: u32,
    pub download_total_tests: u32,
    pub upload_completed_tests: u32,
    pub upload_total_tests: u32,
    pub current_payload_index: usize,
    pub total_payload_sizes: usize,
    pub download_status: String,
    pub upload_status: String,
    pub download_current_speed: f64,
    pub upload_current_speed: f64,
    pub download_average_speed: f64,
    pub upload_average_speed: f64,
    pub download_completed_payload_sizes: usize,
    pub upload_completed_payload_sizes: usize,
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
                download_completed_tests: 0,
                download_total_tests: 0,
                upload_completed_tests: 0,
                upload_total_tests: 0,
                current_payload_index: 0,
                total_payload_sizes: 0,
                download_status: "Waiting...".to_string(),
                upload_status: "Waiting...".to_string(),
                download_current_speed: 0.0,
                upload_current_speed: 0.0,
                download_average_speed: 0.0,
                upload_average_speed: 0.0,
                download_completed_payload_sizes: 0,
                upload_completed_payload_sizes: 0,
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
                        self.progress.download_current_speed = data.speed;
                        self.download_speeds.push_back(data.clone());
                        if self.download_speeds.len() > self.max_data_points {
                            self.download_speeds.pop_front();
                        }

                        // Update status message
                        if let Some(payload_size) = self.progress.current_payload_size {
                            let payload_mb = payload_size / 1_000_000;
                            self.progress.download_status = format!(
                                "Testing {}MB [{}/{}] - Current: {:.1} Mbps",
                                payload_mb,
                                self.progress.current_iteration + 1,
                                self.progress.total_iterations,
                                data.speed
                            );
                        }
                    }
                    TestType::Upload => {
                        self.current_upload_speed = data.speed;
                        self.progress.upload_current_speed = data.speed;
                        self.upload_speeds.push_back(data.clone());
                        if self.upload_speeds.len() > self.max_data_points {
                            self.upload_speeds.pop_front();
                        }

                        // Update status message
                        if let Some(payload_size) = self.progress.current_payload_size {
                            let payload_mb = payload_size / 1_000_000;
                            self.progress.upload_status = format!(
                                "Testing {}MB [{}/{}] - Current: {:.1} Mbps",
                                payload_mb,
                                self.progress.current_iteration + 1,
                                self.progress.total_iterations,
                                data.speed
                            );
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
            TestEvent::TestCompleted(test_type, _) => {
                self.progress.current_iteration += 1;
                match test_type {
                    TestType::Download => {
                        self.progress.download_completed_tests += 1;
                    }
                    TestType::Upload => {
                        self.progress.upload_completed_tests += 1;
                    }
                }
            }
            TestEvent::TestPhaseStarted(test_type, nr_tests, payload_sizes) => {
                self.progress.phase = match test_type {
                    TestType::Download => TestPhase::Download,
                    TestType::Upload => TestPhase::Upload,
                };
                self.progress.total_payload_sizes = payload_sizes.len();
                let total_tests = nr_tests * payload_sizes.len() as u32;
                match test_type {
                    TestType::Download => {
                        self.progress.download_total_tests = total_tests;
                        self.progress.download_completed_tests = 0;
                        self.progress.download_completed_payload_sizes = 0;
                        self.progress.download_status = "Starting...".to_string();
                    }
                    TestType::Upload => {
                        self.progress.upload_total_tests = total_tests;
                        self.progress.upload_completed_tests = 0;
                        self.progress.upload_completed_payload_sizes = 0;
                        self.progress.upload_status = "Starting...".to_string();
                    }
                }
            }
            TestEvent::PayloadSizeStarted(test_type, payload_size, payload_index) => {
                self.progress.current_test = Some(test_type);
                self.progress.current_payload_size = Some(payload_size);
                self.progress.current_payload_index = payload_index;
                self.progress.current_iteration = 0;
                // Calculate total iterations for this payload size
                let total_tests_for_phase = match test_type {
                    TestType::Download => self.progress.download_total_tests,
                    TestType::Upload => self.progress.upload_total_tests,
                };
                self.progress.total_iterations =
                    total_tests_for_phase / self.progress.total_payload_sizes as u32;
            }
            TestEvent::PayloadSizeCompleted(test_type, _) => {
                // Payload size completed, reset current iteration
                self.progress.current_iteration = 0;
                match test_type {
                    TestType::Download => {
                        self.progress.download_completed_payload_sizes += 1;
                    }
                    TestType::Upload => {
                        self.progress.upload_completed_payload_sizes += 1;
                    }
                }
            }
            TestEvent::TestPhaseCompleted(test_type, average_speed) => match test_type {
                TestType::Download => {
                    self.progress.download_average_speed = average_speed;
                    self.progress.download_status = format!(
                        "Completed - Average: {:.1} Mbps ({} payload sizes tested)",
                        average_speed, self.progress.download_completed_payload_sizes
                    );
                }
                TestType::Upload => {
                    self.progress.upload_average_speed = average_speed;
                    self.progress.upload_status = format!(
                        "Completed - Average: {:.1} Mbps ({} payload sizes tested)",
                        average_speed, self.progress.upload_completed_payload_sizes
                    );
                }
            },
            TestEvent::TestsSkipped(test_type, reason) => match test_type {
                TestType::Download => {
                    self.progress.download_status = format!("Skipped ({})", reason);
                }
                TestType::Upload => {
                    self.progress.upload_status = format!("Skipped ({})", reason);
                }
            },
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
                Constraint::Length(4), // Progress bars + Latency stats
                Constraint::Min(10),   // Main content
                Constraint::Length(3), // Status
            ])
            .split(f.area());

        self.draw_title(f, chunks[0]);
        self.draw_progress_and_latency(f, chunks[1]);
        self.draw_main_content(f, chunks[2]);
        self.draw_status(f, chunks[3]);
    }

    fn draw_title(&self, f: &mut Frame, area: Rect) {
        let title_text = if let Some(ref metadata) = self.state.metadata {
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

    fn draw_progress_and_latency(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Download status
                Constraint::Length(1), // Upload status
                Constraint::Length(1), // Latency stats
            ])
            .split(area);

        // Calculate adaptive progress for visual bars
        let download_progress = self.calculate_adaptive_progress(TestType::Download);
        let upload_progress = self.calculate_adaptive_progress(TestType::Upload);

        // Download status with progress bar and text
        let download_style = match self.state.progress.phase {
            TestPhase::Download => ThemedStyles::progress_download_active(),
            TestPhase::Completed if download_progress >= 1.0 => {
                ThemedStyles::progress_download_complete()
            }
            _ if download_progress >= 1.0 => ThemedStyles::progress_download_complete(),
            _ => ThemedStyles::progress_inactive(),
        };

        let download_text = format!(" Download: {}", self.state.progress.download_status);
        let download_paragraph = Paragraph::new(download_text).style(download_style);
        f.render_widget(download_paragraph, chunks[0]);

        // Upload status with progress bar and text
        let upload_style = match self.state.progress.phase {
            TestPhase::Upload => ThemedStyles::progress_upload_active(),
            TestPhase::Completed if upload_progress >= 1.0 => {
                ThemedStyles::progress_upload_complete()
            }
            _ if upload_progress >= 1.0 => ThemedStyles::progress_upload_complete(),
            _ => ThemedStyles::progress_inactive(),
        };

        let upload_text = format!(" Upload:   {}", self.state.progress.upload_status);
        let upload_paragraph = Paragraph::new(upload_text).style(upload_style);
        f.render_widget(upload_paragraph, chunks[1]);

        // Latency stats in a compact single line
        let latency_text = format!(
            " Latency:  Current: {:.1}ms | Average: {:.1}ms | Min/Max: {:.1}ms / {:.1}ms",
            self.state.current_latency,
            self.state.avg_latency,
            if self.state.min_latency == f64::MAX {
                0.0
            } else {
                self.state.min_latency
            },
            self.state.max_latency
        );
        let latency_paragraph = Paragraph::new(latency_text).style(ThemedStyles::latency_stats());
        f.render_widget(latency_paragraph, chunks[2]);
    }

    fn calculate_adaptive_progress(&self, test_type: TestType) -> f64 {
        match test_type {
            TestType::Download => {
                if self.state.progress.download_total_tests > 0 {
                    let completed = self.state.progress.download_completed_tests as f64;
                    let total = self.state.progress.download_total_tests as f64;

                    // Add partial progress for current test if in download phase
                    let current_progress = if self.state.progress.phase == TestPhase::Download {
                        let current_test_progress = if self.state.progress.total_iterations > 0 {
                            self.state.progress.current_iteration as f64
                                / self.state.progress.total_iterations as f64
                        } else {
                            0.0
                        };
                        current_test_progress / total
                    } else {
                        0.0
                    };

                    ((completed + current_progress) / total).min(1.0)
                } else if matches!(
                    self.state.progress.phase,
                    TestPhase::Upload | TestPhase::Completed
                ) {
                    1.0
                } else {
                    0.0
                }
            }
            TestType::Upload => {
                if self.state.progress.upload_total_tests > 0 {
                    let completed = self.state.progress.upload_completed_tests as f64;
                    let total = self.state.progress.upload_total_tests as f64;

                    // Add partial progress for current test if in upload phase
                    let current_progress = if self.state.progress.phase == TestPhase::Upload {
                        let current_test_progress = if self.state.progress.total_iterations > 0 {
                            self.state.progress.current_iteration as f64
                                / self.state.progress.total_iterations as f64
                        } else {
                            0.0
                        };
                        current_test_progress / total
                    } else {
                        0.0
                    };

                    ((completed + current_progress) / total).min(1.0)
                } else if self.state.progress.phase == TestPhase::Completed {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    fn draw_main_content(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        self.draw_speed_graphs(f, chunks[0]);
        self.draw_boxplots(f, chunks[1]);
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
            .border_style(ThemedStyles::download_graph_border());

        let inner = block.inner(area);
        f.render_widget(block, area);

        if !self.state.download_speeds.is_empty() {
            let graph_widget = crate::tui::widgets::LineGraph::new(&self.state.download_speeds)
                .color(TokyoNight::DOWNLOAD_PRIMARY);
            f.render_widget(graph_widget, inner);
        } else {
            let placeholder =
                Paragraph::new("Waiting for data...").style(ThemedStyles::graph_placeholder());
            f.render_widget(placeholder, inner);
        }
    }

    fn draw_upload_graph(&self, f: &mut Frame, area: Rect) {
        let title = format!("Upload Speed ({:.1} Mbps)", self.state.current_upload_speed);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(ThemedStyles::upload_graph_border());

        let inner = block.inner(area);
        f.render_widget(block, area);

        if !self.state.upload_speeds.is_empty() {
            let graph_widget = crate::tui::widgets::LineGraph::new(&self.state.upload_speeds)
                .color(TokyoNight::UPLOAD_PRIMARY);
            f.render_widget(graph_widget, inner);
        } else {
            let placeholder =
                Paragraph::new("Waiting for data...").style(ThemedStyles::graph_placeholder());
            f.render_widget(placeholder, inner);
        }
    }

    fn draw_boxplots(&self, f: &mut Frame, area: Rect) {
        let boxplot_grid = crate::tui::widgets::BoxplotGrid::new(&self.state.measurements);
        f.render_widget(boxplot_grid, area);
    }

    fn draw_status(&self, f: &mut Frame, area: Rect) {
        let (status_text, status_style) = match self.state.progress.phase {
            TestPhase::Idle => (
                " Ready to start tests. Press 'q' to quit.".to_string(),
                ThemedStyles::status_idle(),
            ),
            TestPhase::Latency => (
                " Running latency tests...".to_string(),
                ThemedStyles::status_active(),
            ),
            TestPhase::Download => {
                if let (Some(payload_size), Some(_)) = (
                    self.state.progress.current_payload_size,
                    self.state.progress.current_test,
                ) {
                    (
                        format!(
                            " Testing Download {}MB [{}/{}]",
                            payload_size / 1_000_000,
                            self.state.progress.current_iteration,
                            self.state.progress.total_iterations
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
                if let (Some(payload_size), Some(_)) = (
                    self.state.progress.current_payload_size,
                    self.state.progress.current_test,
                ) {
                    (
                        format!(
                            " Testing Upload {}MB [{}/{}]",
                            payload_size / 1_000_000,
                            self.state.progress.current_iteration,
                            self.state.progress.total_iterations
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
}
