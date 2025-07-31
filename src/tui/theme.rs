use ratatui::style::{Color, Style};

pub struct TokyoNight;

impl TokyoNight {
    // Core Tokyo Night colors
    pub const BACKGROUND: Color = Color::Rgb(26, 27, 38); // #1a1b26
    pub const FOREGROUND: Color = Color::Rgb(192, 202, 245); // #c0caf5
    pub const COMMENT: Color = Color::Rgb(86, 95, 137); // #565f89

    // Accent colors
    pub const PURPLE: Color = Color::Rgb(187, 154, 247); // #bb9af7
    pub const BLUE: Color = Color::Rgb(122, 162, 247); // #7aa2f7
    pub const CYAN: Color = Color::Rgb(125, 207, 255); // #7dcfff
    pub const GREEN: Color = Color::Rgb(158, 206, 106); // #9ece6a
    pub const YELLOW: Color = Color::Rgb(224, 175, 104); // #e0af68
    pub const ORANGE: Color = Color::Rgb(255, 158, 100); // #ff9e64
    pub const RED: Color = Color::Rgb(247, 118, 142); // #f7768e
    pub const MAGENTA: Color = Color::Rgb(187, 154, 247); // #bb9af7

    // UI specific colors
    pub const BORDER: Color = Color::Rgb(86, 95, 137); // #565f89
    pub const BORDER_HIGHLIGHT: Color = Color::Rgb(125, 207, 255); // #7dcfff
    pub const SELECTION: Color = Color::Rgb(41, 46, 66); // #292e42
    pub const VISUAL: Color = Color::Rgb(51, 65, 85); // #334155

    // Status colors
    pub const SUCCESS: Color = Self::GREEN;
    pub const WARNING: Color = Self::YELLOW;
    pub const ERROR: Color = Self::RED;
    pub const INFO: Color = Self::BLUE;

    // Graph colors
    pub const DOWNLOAD_PRIMARY: Color = Self::GREEN;
    pub const DOWNLOAD_SECONDARY: Color = Color::Rgb(134, 180, 92); // Lighter green
    pub const UPLOAD_PRIMARY: Color = Self::BLUE;
    pub const UPLOAD_SECONDARY: Color = Color::Rgb(100, 140, 220); // Lighter blue
    pub const LATENCY_PRIMARY: Color = Self::YELLOW;
    pub const LATENCY_SECONDARY: Color = Color::Rgb(200, 160, 90); // Darker yellow

    // Progress colors
    pub const PROGRESS_COMPLETE: Color = Self::GREEN;
    pub const PROGRESS_ACTIVE: Color = Self::CYAN;
    pub const PROGRESS_PENDING: Color = Self::COMMENT;
    pub const PROGRESS_BACKGROUND: Color = Color::Rgb(41, 46, 66); // #292e42
}

pub struct ThemedStyles;

impl ThemedStyles {
    // Title styles
    pub fn title() -> Style {
        Style::default()
            .fg(TokyoNight::CYAN)
            .bg(TokyoNight::BACKGROUND)
    }

    pub fn title_border() -> Style {
        Style::default().fg(TokyoNight::BORDER_HIGHLIGHT)
    }

    // Progress styles
    pub fn progress_download_active() -> Style {
        Style::default().fg(TokyoNight::DOWNLOAD_PRIMARY)
    }

    pub fn progress_download_complete() -> Style {
        Style::default().fg(TokyoNight::SUCCESS)
    }

    pub fn progress_upload_active() -> Style {
        Style::default().fg(TokyoNight::UPLOAD_PRIMARY)
    }

    pub fn progress_upload_complete() -> Style {
        Style::default().fg(TokyoNight::SUCCESS)
    }

    pub fn progress_inactive() -> Style {
        Style::default().fg(TokyoNight::COMMENT)
    }

    pub fn latency_stats() -> Style {
        Style::default().fg(TokyoNight::LATENCY_PRIMARY)
    }

    // Graph styles
    pub fn download_graph_border() -> Style {
        Style::default().fg(TokyoNight::DOWNLOAD_PRIMARY)
    }

    pub fn upload_graph_border() -> Style {
        Style::default().fg(TokyoNight::UPLOAD_PRIMARY)
    }

    pub fn graph_placeholder() -> Style {
        Style::default().fg(TokyoNight::COMMENT)
    }

    // Boxplot styles
    pub fn boxplot_border() -> Style {
        Style::default().fg(TokyoNight::BORDER)
    }

    pub fn boxplot_download_accent() -> Style {
        Style::default().fg(TokyoNight::DOWNLOAD_PRIMARY)
    }

    pub fn boxplot_upload_accent() -> Style {
        Style::default().fg(TokyoNight::UPLOAD_PRIMARY)
    }

    pub fn boxplot_stats() -> Style {
        Style::default().fg(TokyoNight::FOREGROUND)
    }

    pub fn boxplot_highlight() -> Style {
        Style::default().fg(TokyoNight::CYAN)
    }

    pub fn boxplot_count() -> Style {
        Style::default().fg(TokyoNight::YELLOW)
    }

    // Status styles
    pub fn status_idle() -> Style {
        Style::default().fg(TokyoNight::FOREGROUND)
    }

    pub fn status_active() -> Style {
        Style::default().fg(TokyoNight::CYAN)
    }

    pub fn status_complete() -> Style {
        Style::default().fg(TokyoNight::SUCCESS)
    }

    pub fn status_border() -> Style {
        Style::default().fg(TokyoNight::BORDER)
    }

    // General UI styles
    pub fn default_border() -> Style {
        Style::default().fg(TokyoNight::BORDER)
    }

    pub fn highlight_border() -> Style {
        Style::default().fg(TokyoNight::BORDER_HIGHLIGHT)
    }

    pub fn text_primary() -> Style {
        Style::default().fg(TokyoNight::FOREGROUND)
    }

    pub fn text_secondary() -> Style {
        Style::default().fg(TokyoNight::COMMENT)
    }

    pub fn text_accent() -> Style {
        Style::default().fg(TokyoNight::PURPLE)
    }

    pub fn background() -> Style {
        Style::default().bg(TokyoNight::BACKGROUND)
    }
}

// Progress bar rendering utilities
pub struct ProgressBar;

impl ProgressBar {
    pub fn render_bar(progress: f64, width: usize, active: bool) -> String {
        let filled_width = (progress * width as f64) as usize;
        let empty_width = width.saturating_sub(filled_width);

        let fill_char = if active { '█' } else { '▓' };
        let empty_char = '░';

        format!(
            "{}{}",
            fill_char.to_string().repeat(filled_width),
            empty_char.to_string().repeat(empty_width)
        )
    }

    pub fn render_gradient_bar(progress: f64, width: usize) -> String {
        let filled_width = (progress * width as f64) as usize;
        let mut bar = String::new();

        for i in 0..width {
            if i < filled_width {
                // Use different characters for gradient effect
                let intensity = (i as f64 / width as f64 * 4.0) as usize;
                let char = match intensity {
                    0 => '▏',
                    1 => '▎',
                    2 => '▍',
                    3 => '▌',
                    4 => '▋',
                    5 => '▊',
                    6 => '▉',
                    _ => '█',
                };
                bar.push(char);
            } else {
                bar.push('░');
            }
        }

        bar
    }
}
