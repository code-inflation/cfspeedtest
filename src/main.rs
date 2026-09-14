use cfspeedtest::speedtest;
use cfspeedtest::OutputFormat;
use cfspeedtest::SpeedTestCLIOptions;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::io;
use std::net::IpAddr;

use cfspeedtest::run::RunConfig;
use speedtest::speed_test_with_config;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Parser)]
#[command(version, about = "Unofficial CLI for speed.cloudflare.com")]
struct CliOptions {
    #[command(flatten)]
    options: SpeedTestCLIOptions,
    /// Base URL of a compatible Cloudflare speed-test service
    #[arg(long, default_value = "https://speed.cloudflare.com")]
    server: reqwest::Url,
    /// Maximum total run duration in seconds
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=86400))]
    max_duration: u64,
    /// Maximum cumulative retry wait in seconds
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(0..=86400))]
    max_retry_wait: u64,
}

fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() -> ExitCode {
    env_logger::init();
    let cli = CliOptions::parse();
    let options = cli.options;

    if let Some(generator) = options.completion {
        let mut cmd = CliOptions::command();
        eprintln!("Generating completion script for {generator}...");
        print_completions(generator, &mut cmd);
        return ExitCode::SUCCESS;
    }

    if options.output_format == OutputFormat::StdOut {
        println!("Starting Cloudflare speed test");
    }
    let client;
    if let Some(ref ip) = options.ipv4 {
        client = reqwest::blocking::Client::builder()
            .local_address(ip.parse::<IpAddr>().expect("Invalid IPv4 address"))
            .timeout(std::time::Duration::from_secs(30))
            .cookie_store(true)
            .build();
    } else if let Some(ref ip) = options.ipv6 {
        client = reqwest::blocking::Client::builder()
            .local_address(ip.parse::<IpAddr>().expect("Invalid IPv6 address"))
            .timeout(std::time::Duration::from_secs(30))
            .cookie_store(true)
            .build();
    } else {
        client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .cookie_store(true)
            .build();
    }
    let config = RunConfig {
        base_url: cli.server.as_str().trim_end_matches('/').to_string(),
        max_duration: Duration::from_secs(cli.max_duration),
        max_retry_wait: Duration::from_secs(cli.max_retry_wait),
        ..RunConfig::default()
    };
    let cancelled = config.cancelled.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        cancelled.store(true, Ordering::SeqCst);
    }) {
        eprintln!("Failed to install cancellation handler: {error}");
        return ExitCode::FAILURE;
    }
    let report = speed_test_with_config(
        client.expect("Failed to initialize reqwest client"),
        options,
        config,
    );
    for error in &report.errors {
        let payload = error
            .payload_size
            .map_or(String::new(), |bytes| format!(" ({bytes} bytes)"));
        eprintln!("{}{}: {}", error.stage, payload, error.error);
    }
    if let Some(reason) = report.stop_reason {
        eprintln!("Run stopped: {reason:?}");
    }
    if report.exit_code() != 0 {
        eprintln!(
            "Run status: {:?}; completed measurements are retained in the output",
            report.status
        );
    }
    ExitCode::from(report.exit_code())
}
