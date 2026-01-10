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

    let client = match build_http_client(&options) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    speed_test(client, options);
}

fn build_http_client(options: &SpeedTestCLIOptions) -> Result<reqwest::blocking::Client, String> {
    let mut builder =
        reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(30));

    if let Some(ref ip) = options.ipv4 {
        let ip_addr = ip
            .parse::<IpAddr>()
            .map_err(|e| format!("Invalid IPv4 address '{}': {}", ip, e))?;
        builder = builder.local_address(ip_addr);
    } else if let Some(ref ip) = options.ipv6 {
        let ip_addr = ip
            .parse::<IpAddr>()
            .map_err(|e| format!("Invalid IPv6 address '{}': {}", ip, e))?;
        builder = builder.local_address(ip_addr);
    }

    builder
        .build()
        .map_err(|e| format!("Failed to initialize HTTP client: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_http_client_invalid_ipv4() {
        let options = SpeedTestCLIOptions {
            ipv4: Some("invalid-ip".to_string()),
            ipv6: None,
            ..Default::default()
        };

        let result = build_http_client(&options);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid IPv4 address"));
        assert!(err.contains("invalid-ip"));
    }

    #[test]
    fn test_build_http_client_invalid_ipv6() {
        let options = SpeedTestCLIOptions {
            ipv4: None,
            ipv6: Some("invalid-ipv6".to_string()),
            ..Default::default()
        };

        let result = build_http_client(&options);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Invalid IPv6 address"));
        assert!(err.contains("invalid-ipv6"));
    }

    #[test]
    fn test_build_http_client_valid_ipv4() {
        let options = SpeedTestCLIOptions {
            ipv4: Some("127.0.0.1".to_string()),
            ipv6: None,
            ..Default::default()
        };

        let result = build_http_client(&options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_http_client_valid_ipv6() {
        let options = SpeedTestCLIOptions {
            ipv4: None,
            ipv6: Some("::1".to_string()),
            ..Default::default()
        };

        let result = build_http_client(&options);
        assert!(result.is_ok());
    }

    #[test]
    fn test_build_http_client_no_ip() {
        let options = SpeedTestCLIOptions {
            ipv4: None,
            ipv6: None,
            ..Default::default()
        };

        let result = build_http_client(&options);
        assert!(result.is_ok());
    }
}
