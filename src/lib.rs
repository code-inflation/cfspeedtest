pub mod boxplot;
pub mod measurements;
pub mod progress;
pub mod speedtest;
use std::fmt;
use std::fmt::Display;

use clap::Parser;
use speedtest::PayloadSize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Csv,
    Json,
    JsonPretty,
    StdOut,
    None,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl OutputFormat {
    pub fn from(output_format_string: String) -> Result<Self, String> {
        match output_format_string.to_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "json_pretty" | "json-pretty" => Ok(Self::JsonPretty),
            "stdout" => Ok(Self::StdOut),
            _ => Err("Value needs to be one of csv, json or json-pretty".to_string()),
        }
    }
}

/// Unofficial CLI for speed.cloudflare.com
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct SpeedTestCLIOptions {
    /// Number of test runs per payload size. Needs to be at least 4
    #[arg(value_parser = clap::value_parser!(u32).range(4..1000), short, long, default_value_t = 10)]
    pub nr_tests: u32,

    /// Number of latency tests to run
    #[arg(long, default_value_t = 25)]
    pub nr_latency_tests: u32,

    /// The max payload size in bytes to use [100k, 1m, 10m, 25m or 100m]
    #[arg(value_parser = parse_payload_size, short, long, default_value_t = PayloadSize::M25)]
    pub max_payload_size: PayloadSize,

    /// Set the output format [csv, json or json-pretty] >
    /// This silences all other output to stdout
    #[arg(value_parser = parse_output_format, short, long, default_value_t = OutputFormat::StdOut)]
    pub output_format: OutputFormat,

    /// Enable verbose output i.e. print boxplots of the measurements
    #[arg(short, long)]
    pub verbose: bool,

    /// Force usage of IPv4
    #[arg(long)]
    pub ipv4: bool,

    /// Force usage of IPv6
    #[arg(long)]
    pub ipv6: bool,

    /// Disables dynamically skipping tests with larger payload sizes if the tests for the previous payload
    /// size took longer than 5 seconds
    #[arg(short, long)]
    pub disable_dynamic_max_payload_size: bool,

    /// Test download speed only
    #[arg(long)]
    pub download_only: bool,

    /// Test upload speed only
    #[arg(long)]
    pub upload_only: bool,
}

impl SpeedTestCLIOptions {
    /// Returns whether download tests should be performed
    pub fn should_download(&self) -> bool {
        self.download_only || (!self.download_only && !self.upload_only)
    }

    /// Returns whether upload tests should be performed
    pub fn should_upload(&self) -> bool {
        self.upload_only || (!self.download_only && !self.upload_only)
    }
}

fn parse_payload_size(input_string: &str) -> Result<PayloadSize, String> {
    PayloadSize::from(input_string.to_string())
}

fn parse_output_format(input_string: &str) -> Result<OutputFormat, String> {
    OutputFormat::from(input_string.to_string())
}
