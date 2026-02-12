use clap::{Parser, ValueEnum};
use clap_complete::Shell;

use crate::engine::types::{PayloadSize, SpeedTestConfig};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MaxPayloadArg {
    #[value(name = "100k")]
    K100,
    #[value(name = "1m")]
    M1,
    #[value(name = "10m")]
    M10,
    #[value(name = "25m")]
    M25,
    #[value(name = "100m")]
    M100,
}

impl From<MaxPayloadArg> for PayloadSize {
    fn from(arg: MaxPayloadArg) -> Self {
        match arg {
            MaxPayloadArg::K100 => PayloadSize::K100,
            MaxPayloadArg::M1 => PayloadSize::M1,
            MaxPayloadArg::M10 => PayloadSize::M10,
            MaxPayloadArg::M25 => PayloadSize::M25,
            MaxPayloadArg::M100 => PayloadSize::M100,
        }
    }
}

/// Which output mode was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Tui,
    Simple,
    Json,
    JsonPretty,
    Csv,
}

/// Unofficial CLI for speed.cloudflare.com
#[derive(Parser, Debug)]
#[command(name = "cfspeedtest", version, about)]
pub struct Cli {
    /// Number of tests per payload size
    #[arg(short = 'n', long = "nr-tests", default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=999))]
    pub nr_tests: u32,

    /// Number of latency tests
    #[arg(long = "nr-latency-tests", default_value_t = 25)]
    pub nr_latency_tests: u32,

    /// Maximum payload size
    #[arg(short = 'p', long = "max-payload-size", default_value = "25m")]
    pub max_payload_size: MaxPayloadArg,

    /// One-line output (no TUI)
    #[arg(long)]
    pub simple: bool,

    /// JSON output
    #[arg(long)]
    pub json: bool,

    /// Pretty JSON output
    #[arg(long = "json-pretty")]
    pub json_pretty: bool,

    /// CSV output
    #[arg(long)]
    pub csv: bool,

    /// Skip upload tests
    #[arg(long = "download-only")]
    pub download_only: bool,

    /// Skip download tests
    #[arg(long = "upload-only")]
    pub upload_only: bool,

    /// Force IPv4 with optional source address
    #[arg(long, num_args = 0..=1, default_missing_value = "0.0.0.0")]
    pub ipv4: Option<String>,

    /// Force IPv6 with optional source address
    #[arg(long, num_args = 0..=1, default_missing_value = "::")]
    pub ipv6: Option<String>,

    /// Disable dynamic max payload size (skip if test > 5s)
    #[arg(short = 'd', long = "disable-dynamic-max-payload-size")]
    pub disable_dynamic_max_payload_size: bool,

    /// Generate shell completions
    #[arg(long = "generate-completion", value_name = "SHELL")]
    pub completion: Option<Shell>,
}

impl Cli {
    pub fn output_mode(&self) -> OutputMode {
        if self.simple {
            OutputMode::Simple
        } else if self.json {
            OutputMode::Json
        } else if self.json_pretty {
            OutputMode::JsonPretty
        } else if self.csv {
            OutputMode::Csv
        } else {
            OutputMode::Tui
        }
    }

    pub fn to_config(&self) -> SpeedTestConfig {
        SpeedTestConfig {
            nr_tests: self.nr_tests,
            nr_latency_tests: self.nr_latency_tests,
            max_payload_size: self.max_payload_size.into(),
            disable_dynamic_max_payload_size: self.disable_dynamic_max_payload_size,
            download: !self.upload_only,
            upload: !self.download_only,
        }
    }
}
