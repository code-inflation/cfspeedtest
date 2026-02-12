use ratatui::style::Color;

/// Cloudflare orange for download indicators.
pub const DOWNLOAD_COLOR: Color = Color::Rgb(245, 135, 0);

/// Blue for upload indicators.
pub const UPLOAD_COLOR: Color = Color::Rgb(100, 180, 255);

/// Green for latency indicators.
pub const LATENCY_COLOR: Color = Color::Rgb(130, 220, 130);

/// Muted text.
pub const DIM_TEXT: Color = Color::DarkGray;

/// Bright white for hero numbers.
pub const BRIGHT_TEXT: Color = Color::White;

/// Border color.
pub const BORDER_COLOR: Color = Color::Rgb(80, 80, 80);

/// Header accent.
pub const HEADER_COLOR: Color = Color::Rgb(245, 135, 0);
