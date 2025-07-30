use crate::measurements::{format_bytes, Measurement};
use crate::speedtest::TestType;
use crate::tui::app::SpeedData;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Line as CanvasLine, Points},
        Block, Borders, Paragraph, Widget,
    },
};
use std::collections::{HashMap, VecDeque};

pub struct LineGraph<'a> {
    data: &'a VecDeque<SpeedData>,
    color: Color,
}

impl<'a> LineGraph<'a> {
    pub fn new(data: &'a VecDeque<SpeedData>) -> Self {
        Self {
            data,
            color: Color::White,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl<'a> Widget for LineGraph<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.data.is_empty() {
            return;
        }

        let max_speed = self
            .data
            .iter()
            .map(|d| d.speed)
            .fold(0.0f64, f64::max)
            .max(1.0); // Ensure minimum scale

        let min_speed = 0.0;
        let speed_range = max_speed - min_speed;

        let points: Vec<(f64, f64)> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, data)| {
                let x = i as f64;
                let y = (data.speed - min_speed) / speed_range * 100.0;
                (x, y)
            })
            .collect();

        if points.len() < 2 {
            return;
        }

        let canvas = Canvas::default()
            .x_bounds([0.0, (self.data.len() - 1) as f64])
            .y_bounds([0.0, 100.0])
            .paint(|ctx| {
                // Draw the line graph
                for window in points.windows(2) {
                    if let [p1, p2] = window {
                        ctx.draw(&CanvasLine {
                            x1: p1.0,
                            y1: p1.1,
                            x2: p2.0,
                            y2: p2.1,
                            color: self.color,
                        });
                    }
                }

                // Draw points
                ctx.draw(&Points {
                    coords: &points,
                    color: self.color,
                });
            });

        canvas.render(area, buf);
    }
}

pub struct SimpleLineChart<'a> {
    data: &'a [f64],
    color: Color,
    max_value: Option<f64>,
}

impl<'a> SimpleLineChart<'a> {
    pub fn new(data: &'a [f64]) -> Self {
        Self {
            data,
            color: Color::White,
            max_value: None,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn max_value(mut self, max_value: f64) -> Self {
        self.max_value = Some(max_value);
        self
    }
}

impl<'a> Widget for SimpleLineChart<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.data.is_empty() || area.height < 2 {
            return;
        }

        let max_val = self
            .max_value
            .unwrap_or_else(|| self.data.iter().fold(0.0f64, |a, &b| a.max(b)).max(1.0));

        let width = area.width as usize;
        let height = area.height as usize;

        // Sample data to fit the width
        let step = if self.data.len() > width {
            self.data.len() / width
        } else {
            1
        };

        let sampled_data: Vec<f64> = self
            .data
            .iter()
            .step_by(step)
            .take(width)
            .cloned()
            .collect();

        // Draw the line chart using simple characters
        for (i, &value) in sampled_data.iter().enumerate() {
            if i >= width {
                break;
            }

            let normalized = (value / max_val).min(1.0);
            let bar_height = (normalized * (height - 1) as f64) as usize;

            for y in 0..height {
                let screen_y = area.y + (height - 1 - y) as u16;
                let screen_x = area.x + i as u16;

                if y <= bar_height {
                    if let Some(cell) = buf.cell_mut((screen_x, screen_y)) {
                        cell.set_char('█').set_fg(self.color);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoxplotData {
    pub test_type: TestType,
    pub payload_size: usize,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub avg: f64,
    pub count: usize,
}

impl BoxplotData {
    pub fn from_measurements(
        measurements: &[Measurement],
        test_type: TestType,
        payload_size: usize,
    ) -> Option<Self> {
        let filtered: Vec<f64> = measurements
            .iter()
            .filter(|m| m.test_type == test_type && m.payload_size == payload_size)
            .map(|m| m.mbit)
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let (min, q1, median, q3, max, avg) = crate::measurements::calc_stats(filtered.clone())?;

        Some(BoxplotData {
            test_type,
            payload_size,
            min,
            q1,
            median,
            q3,
            max,
            avg,
            count: filtered.len(),
        })
    }

    pub fn title(&self) -> String {
        format!("{:?} {}", self.test_type, format_bytes(self.payload_size))
    }

    pub fn color(&self) -> Color {
        match self.test_type {
            TestType::Download => Color::Green,
            TestType::Upload => Color::Blue,
        }
    }
}

pub struct BoxplotWidget<'a> {
    data: &'a BoxplotData,
    width: u16,
}

impl<'a> BoxplotWidget<'a> {
    pub fn new(data: &'a BoxplotData) -> Self {
        Self {
            data,
            width: 40, // Default width
        }
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    fn render_boxplot_line(&self, area_width: u16) -> String {
        let width = (area_width.saturating_sub(2)) as usize; // Account for borders
        if width < 10 {
            return "Too narrow".to_string();
        }

        let range = self.data.max - self.data.min;
        if range == 0.0 {
            // All values are the same
            let middle = width / 2;
            let mut line = vec![' '; width];
            if middle < width {
                line[middle] = '│';
            }
            return line.into_iter().collect();
        }

        let scale = (width - 1) as f64 / range;

        // Calculate positions
        let min_pos = 0;
        let q1_pos = ((self.data.q1 - self.data.min) * scale) as usize;
        let median_pos = ((self.data.median - self.data.min) * scale) as usize;
        let q3_pos = ((self.data.q3 - self.data.min) * scale) as usize;
        let max_pos = width - 1;

        let mut line = vec![' '; width];

        // Draw whiskers
        for item in line
            .iter_mut()
            .take(q1_pos.min(width - 1) + 1)
            .skip(min_pos)
        {
            *item = '─';
        }
        for item in line
            .iter_mut()
            .take(max_pos.min(width - 1) + 1)
            .skip(q3_pos)
        {
            *item = '─';
        }

        // Draw box
        for item in line.iter_mut().take(q3_pos.min(width - 1) + 1).skip(q1_pos) {
            *item = '█';
        }

        // Draw markers
        if min_pos < width {
            line[min_pos] = '├';
        }
        if q1_pos < width {
            line[q1_pos] = '┤';
        }
        if median_pos < width {
            line[median_pos] = '│';
        }
        if q3_pos < width {
            line[q3_pos] = '├';
        }
        if max_pos < width {
            line[max_pos] = '┤';
        }

        line.into_iter().collect()
    }
}

impl<'a> Widget for BoxplotWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.data.title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.data.color()));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 4 {
            return; // Not enough space
        }

        // Create content lines
        let boxplot_line = self.render_boxplot_line(inner.width);

        let content = vec![
            Line::from(vec![
                Span::raw("Count: "),
                Span::styled(
                    format!("{}", self.data.count),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(boxplot_line),
            Line::from(vec![
                Span::raw("Min: "),
                Span::styled(
                    format!("{:.1}", self.data.min),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" Max: "),
                Span::styled(
                    format!("{:.1}", self.data.max),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::raw("Avg: "),
                Span::styled(
                    format!("{:.1}", self.data.avg),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" Med: "),
                Span::styled(
                    format!("{:.1}", self.data.median),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(content);
        paragraph.render(inner, buf);
    }
}

pub struct BoxplotGrid {
    boxplots: Vec<BoxplotData>,
}

impl BoxplotGrid {
    pub fn new(measurements: &[Measurement]) -> Self {
        let mut boxplots = Vec::new();
        let mut combinations = HashMap::new();

        // Find all unique test_type + payload_size combinations
        for measurement in measurements {
            combinations.insert((measurement.test_type, measurement.payload_size), ());
        }

        // Create boxplot data for each combination
        for (test_type, payload_size) in combinations.keys() {
            if let Some(boxplot_data) =
                BoxplotData::from_measurements(measurements, *test_type, *payload_size)
            {
                boxplots.push(boxplot_data);
            }
        }

        // Sort by test type first, then by payload size
        boxplots.sort_by(|a, b| match a.test_type.cmp(&b.test_type) {
            std::cmp::Ordering::Equal => a.payload_size.cmp(&b.payload_size),
            other => other,
        });

        Self { boxplots }
    }
}

impl Widget for BoxplotGrid {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.boxplots.is_empty() {
            let placeholder = Paragraph::new("No measurement data available yet...")
                .style(Style::default().fg(Color::Gray))
                .block(
                    Block::default()
                        .title("Measurement Boxplots")
                        .borders(Borders::ALL),
                );
            placeholder.render(area, buf);
            return;
        }

        let block = Block::default()
            .title("Measurement Boxplots")
            .borders(Borders::ALL);

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate layout - try to fit boxplots in a grid
        let boxplot_count = self.boxplots.len();
        if boxplot_count == 0 {
            return;
        }

        // Determine grid dimensions based on available space and number of boxplots
        let min_boxplot_height = 6; // Minimum height needed for a boxplot
        let min_boxplot_width = 25; // Minimum width needed for a boxplot

        let max_rows = (inner.height / min_boxplot_height as u16).max(1) as usize;
        let max_cols = (inner.width / min_boxplot_width as u16).max(1) as usize;

        let cols = (boxplot_count as f64).sqrt().ceil() as usize;
        let cols = cols.min(max_cols).max(1);
        let rows = boxplot_count.div_ceil(cols).min(max_rows);

        // Create constraints for rows and columns
        let row_constraints: Vec<Constraint> = (0..rows)
            .map(|_| Constraint::Length(inner.height / rows as u16))
            .collect();

        let col_constraints: Vec<Constraint> = (0..cols)
            .map(|_| Constraint::Percentage(100 / cols as u16))
            .collect();

        // Create row layout
        let row_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(inner);

        // Render boxplots in grid
        for (row_idx, row_area) in row_chunks.iter().enumerate().take(rows) {
            if row_idx * cols >= boxplot_count {
                break;
            }

            let col_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(col_constraints.clone())
                .split(*row_area);

            for (col_idx, col_area) in col_chunks.iter().enumerate().take(cols) {
                let boxplot_idx = row_idx * cols + col_idx;
                if boxplot_idx >= boxplot_count {
                    break;
                }

                let boxplot_widget = BoxplotWidget::new(&self.boxplots[boxplot_idx]);
                boxplot_widget.render(*col_area, buf);
            }
        }
    }
}
