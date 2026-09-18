use cfspeedtest::speedtest;
use cfspeedtest::stdout;
use cfspeedtest::OutputFormat;
use cfspeedtest::SpeedTestCLIOptions;
use clap::{CommandFactory, Parser};
use clap_complete::generate;

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
    // Buffer the script in memory: clap_complete panics on write errors, and
    // piping the output into e.g. `head` must not abort the process.
    let mut buffer = Vec::new();
    generate(gen, cmd, cmd.get_name().to_string(), &mut buffer);
    stdout::print(&String::from_utf8_lossy(&buffer));
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
        stdout::print_line("Starting Cloudflare speed test");
    }
    let local_address = match cfspeedtest::parse_bound_address(&options.ipv4, &options.ipv6) {
        Ok(address) => address,
        Err(message) => {
            eprintln!("Error: {message}");
            return ExitCode::FAILURE;
        }
    };
    let mut client_builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .cookie_store(true);
    if let Some(address) = local_address {
        client_builder = client_builder.local_address(address);
    }
    let client = client_builder.build();
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
