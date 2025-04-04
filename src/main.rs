use cfspeedtest::speedtest;
use cfspeedtest::OutputFormat;
use cfspeedtest::SpeedTestCLIOptions;
use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::io;
use std::net::IpAddr;

use speedtest::speed_test;

fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn main() {
    env_logger::init();
    let options = SpeedTestCLIOptions::parse();

    if let Some(generator) = options.completion {
        let mut cmd = SpeedTestCLIOptions::command();
        eprintln!("Generating completion script for {generator}...");
        print_completions(generator, &mut cmd);
        return;
    }

    if options.output_format == OutputFormat::StdOut {
        println!("Starting Cloudflare speed test");
    }
    let client;
    if options.ipv4 {
        client = reqwest::blocking::Client::builder()
            .local_address("0.0.0.0".parse::<IpAddr>().unwrap())
            .build();
    } else if options.ipv6 {
        client = reqwest::blocking::Client::builder()
            .local_address("::".parse::<IpAddr>().unwrap())
            .build();
    } else {
        client = reqwest::blocking::Client::builder().build();
    }
    speed_test(
        client.expect("Failed to initialize reqwest client"),
        options,
    );
}
