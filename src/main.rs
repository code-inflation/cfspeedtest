use cfspeedtest::speedtest;
use cfspeedtest::SpeedTestCLIOptions;
use clap::Parser;
use std::net::IpAddr;

use speedtest::speed_test;

fn main() {
    env_logger::init();
    let options = SpeedTestCLIOptions::parse();
    if options.output_format.is_none() {
        println!("Starting Cloudflare speed test");
    }
    let client;
    if options.ipv4 {
        client = reqwest::blocking::Client::builder()
            .local_address("0.0.0.0".parse::<IpAddr>().unwrap())
            .build();
    } else if options.ipv6 {
        client = reqwest::blocking::Client::builder()
            .local_address("::1".parse::<IpAddr>().unwrap())
            .build();
    } else {
        client = reqwest::blocking::Client::builder().build();
    }
    speed_test(
        client.expect("Failed to initialize reqwest client"),
        options,
    );
}
