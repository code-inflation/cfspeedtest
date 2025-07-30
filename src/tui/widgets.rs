use crate::tui::app::SpeedData;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    widgets::{
        canvas::{Canvas, Line, Points},
        Widget,
    },
};
use std::collections::VecDeque;

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
                        ctx.draw(&Line {
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
