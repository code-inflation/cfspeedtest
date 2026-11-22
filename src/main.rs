use cfspeedtest::run::RunConfig;
use cfspeedtest::speedtest::speed_test_with_config;
use cfspeedtest::stdout;
use cfspeedtest::{OutputFormat, SpeedTestCLIOptions};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::net::IpAddr;
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

fn print_completions(generator: clap_complete::Shell) {
    let mut cmd = CliOptions::command();
    let name = cmd.get_name().to_owned();
    // clap_complete's generators assume writes succeed.
    let mut buffer = Vec::new();
    generate(generator, &mut cmd, name, &mut buffer);
    stdout::write(|stdout| stdout.write_all(&buffer));
}

fn local_address(options: &SpeedTestCLIOptions) -> Option<IpAddr> {
    let (flag, family, input, address) = if let Some(input) = &options.ipv4 {
        ("--ipv4", "IPv4", input, input.parse().map(IpAddr::V4))
    } else if let Some(input) = &options.ipv6 {
        ("--ipv6", "IPv6", input, input.parse().map(IpAddr::V6))
    } else {
        return None;
    };
    match address {
        Ok(address) => Some(address),
        Err(_) => CliOptions::command()
            .error(
                ErrorKind::ValueValidation,
                format!("invalid {family} address '{input}' for {flag}"),
            )
            .exit(),
    }
}

fn print_diagnostic(args: fmt::Arguments<'_>) {
    let _ = writeln!(io::stderr().lock(), "{args}");
}

fn main() -> ExitCode {
    env_logger::init();
    let cli = CliOptions::parse();
    let options = cli.options;
    let local_address = local_address(&options);

    if let Some(generator) = options.completion {
        print_diagnostic(format_args!(
            "Generating completion script for {generator}..."
        ));
        print_completions(generator);
        return ExitCode::SUCCESS;
    }

    let mut client_builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .cookie_store(true);
    if let Some(address) = local_address {
        client_builder = client_builder.local_address(address);
    }
    let client = match client_builder.build() {
        Ok(client) => client,
        Err(error) => {
            print_diagnostic(format_args!("Failed to initialize HTTP client: {error}"));
            if let Some(source) = error.source() {
                print_diagnostic(format_args!("  {source}"));
            }
            return ExitCode::FAILURE;
        }
    };
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
        print_diagnostic(format_args!(
            "Failed to install cancellation handler: {error}"
        ));
        return ExitCode::FAILURE;
    }
    if options.output_format == OutputFormat::StdOut {
        stdout::write(|stdout| writeln!(stdout, "Starting Cloudflare speed test"));
    }
    let report = speed_test_with_config(client, options, config);
    for error in &report.errors {
        let payload = error
            .payload_size
            .map_or(String::new(), |bytes| format!(" ({bytes} bytes)"));
        print_diagnostic(format_args!("{}{}: {}", error.stage, payload, error.error));
    }
    if let Some(reason) = report.stop_reason {
        print_diagnostic(format_args!("Run stopped: {reason:?}"));
    }
    let exit_code = report.exit_code();
    if exit_code != 0 {
        print_diagnostic(format_args!(
            "Run status: {:?}; completed measurements are retained in the output",
            report.status
        ));
    }
    ExitCode::from(exit_code)
}
