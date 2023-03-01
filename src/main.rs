pub mod boxplot;
pub mod measurements;
pub mod progress;
pub mod speedtest;

use clap::Parser;
use speedtest::speed_test;
use speedtest::PayloadSize;

#[derive(Clone, Copy, Debug)]
enum OutputFormat {
    Csv,
    Json,
    JsonPretty,
}

impl OutputFormat {
    pub fn from(output_format_string: String) -> Result<Self, String> {
        match output_format_string.to_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "json_pretty" | "json-pretty" => Ok(Self::JsonPretty),
            _ => Err("Value needs to be one of csv, json or json-pretty".to_string()),
        }
    }
}

/// Unofficial CLI for speed.cloudflare.com
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct SpeedTestOptions {
    /// Number of test runs per payload size. Needs to be at least 4
    #[arg(value_parser = clap::value_parser!(u32).range(4..1000), short, long, default_value_t = 10)]
    nr_tests: u32,

    /// Number of latency tests to run
    #[arg(long, default_value_t = 25)]
    nr_latency_tests: u32,

    /// The max payload size in bytes to use [100k, 1m, 10m, 25m or 100m]
    #[arg(value_parser = parse_payload_size, short, long, default_value_t = PayloadSize::M10)]
    max_payload_size: PayloadSize,

    /// Set the output format [csv, json or json-pretty] >
    /// This silences all other output to stdout
    #[arg(value_parser = parse_output_format, short, long)]
    outupt_format: Option<OutputFormat>,

    /// Enable verbose output i.e. print out boxplots of the measurements
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    env_logger::init();
    let options = SpeedTestOptions::parse();
    if options.outupt_format.is_none() {
        println!("Starting Cloudflare speed test");
    }
    let client = reqwest::blocking::Client::new();
    speed_test(client, options);
}

fn parse_payload_size(input_string: &str) -> Result<PayloadSize, String> {
    PayloadSize::from(input_string.to_string())
}

fn parse_output_format(input_string: &str) -> Result<OutputFormat, String> {
    OutputFormat::from(input_string.to_string())
}
