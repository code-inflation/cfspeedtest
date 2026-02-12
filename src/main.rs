use std::net::IpAddr;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use tokio::sync::mpsc;

use cfspeedtest::cli::{Cli, OutputMode};
use cfspeedtest::engine::client::build_client;
use cfspeedtest::engine::runner::run_speed_test;
use cfspeedtest::engine::types::SpeedTestEvent;
use cfspeedtest::output;

fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Shell completions
    if let Some(generator) = cli.completion {
        let mut cmd = Cli::command();
        eprintln!("Generating completion script for {generator}...");
        print_completions(generator, &mut cmd);
        return Ok(());
    }

    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // Resolve local address
    let local_addr: Option<IpAddr> = if let Some(ref ip) = cli.ipv4 {
        Some(ip.parse().context("Invalid IPv4 address")?)
    } else if let Some(ref ip) = cli.ipv6 {
        Some(ip.parse().context("Invalid IPv6 address")?)
    } else {
        None
    };

    let client = build_client(local_addr).context("Failed to build HTTP client")?;
    let config = cli.to_config();
    let mode = cli.output_mode();

    match mode {
        OutputMode::Tui => {
            cfspeedtest::tui::run(client, config).await?;
        }
        _ => {
            // Headless mode: run engine, collect final result
            let (tx, mut rx) = mpsc::channel::<SpeedTestEvent>(256);

            let engine_client = client.clone();
            let engine_config = config.clone();
            let handle =
                tokio::spawn(
                    async move { run_speed_test(&engine_client, &engine_config, tx).await },
                );

            // Drain events (for simple mode, show progress)
            let mut result = None;
            while let Some(event) = rx.recv().await {
                if let SpeedTestEvent::Complete(r) = event {
                    result = Some(r);
                }
            }

            let result = match result {
                Some(r) => r,
                None => handle.await??,
            };

            match mode {
                OutputMode::Simple => output::simple::print_simple(&result),
                OutputMode::Json => output::json::print_json(&result),
                OutputMode::JsonPretty => output::json::print_json_pretty(&result),
                OutputMode::Csv => output::csv::print_csv(&result),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
