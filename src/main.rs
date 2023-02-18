pub mod boxplot;
pub mod measurements;
pub mod progress;
pub mod speedtest;

use clap::Parser;
use speedtest::speed_test;

pub const PAYLOAD_SIZES: [usize; 4] = [100_000, 1_000_000, 10_000_000, 25_000_000];

/// Unofficial CLI for speed.cloudflare.com
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of test runs per payload size. Needs to be at least 4
    #[arg(value_parser = clap::value_parser!(u32).range(4..1000), short, long, default_value_t = 10)]
    nr_tests: u32,

    /// Number of latency tests to run
    #[arg(long, default_value_t = 25)]
    nr_latency_tests: u32,
}

fn main() {
    env_logger::init();
    let args = Args::parse();
    println!("Starting Cloudflare speed test");
    let client = reqwest::blocking::Client::new();
    speed_test(client, args.nr_tests, args.nr_latency_tests);
}
