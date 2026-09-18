pub mod boxplot;
pub mod measurements;
pub mod progress;
pub mod run;
pub mod speedtest;
pub mod stdout;
use std::fmt;
use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use clap::Parser;
use clap_complete::Shell;
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
        write!(f, "{self:?}")
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
    /// Number of test runs per payload size.
    #[arg(value_parser = clap::value_parser!(u32).range(1..1000), short, long, default_value_t = 10)]
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

    /// Force IPv4 with provided source IPv4 address or the default IPv4 address bound to the main interface
    #[clap(long, value_name = "IPv4", num_args = 0..=1, default_missing_value = "0.0.0.0", value_parser = parse_ipv4_address, conflicts_with = "ipv6")]
    pub ipv4: Option<String>,

    /// Force IPv6 with provided source IPv6 address or the default IPv6 address bound to the main interface
    #[clap(long, value_name = "IPv6", num_args = 0..=1, default_missing_value = "::", value_parser = parse_ipv6_address, conflicts_with = "ipv4")]
    pub ipv6: Option<String>,

    /// Disables dynamically skipping tests with larger payload sizes if the tests for the previous payload
    /// size took longer than 5 seconds
    #[arg(short, long)]
    pub disable_dynamic_max_payload_size: bool,

    /// Test download speed only
    #[arg(long, conflicts_with = "upload_only")]
    pub download_only: bool,

    /// Test upload speed only
    #[arg(long, conflicts_with = "download_only")]
    pub upload_only: bool,

    /// Generate shell completion script for the specified shell
    #[arg(long = "generate-completion", value_enum)]
    pub completion: Option<Shell>,
}

impl SpeedTestCLIOptions {
    /// Returns whether download tests should be performed
    pub fn should_download(&self) -> bool {
        self.download_only || !self.upload_only
    }

    /// Returns whether upload tests should be performed
    pub fn should_upload(&self) -> bool {
        self.upload_only || !self.download_only
    }
}

fn parse_payload_size(input_string: &str) -> Result<PayloadSize, String> {
    PayloadSize::from(input_string.to_string())
}

fn parse_ipv4_address(input_string: &str) -> Result<String, String> {
    match input_string.parse::<Ipv4Addr>() {
        Ok(_) => Ok(input_string.to_string()),
        Err(_) => Err(format!("invalid IPv4 address: '{input_string}'")),
    }
}

fn parse_ipv6_address(input_string: &str) -> Result<String, String> {
    match input_string.parse::<Ipv6Addr>() {
        Ok(_) => Ok(input_string.to_string()),
        Err(_) => Err(format!("invalid IPv6 address: '{input_string}'")),
    }
}

fn parse_output_format(input_string: &str) -> Result<OutputFormat, String> {
    OutputFormat::from(input_string.to_string())
}

/// Parses the bound address from the CLI options. Returns an error if the
/// provided address is not a valid IP address.
pub fn parse_bound_address(
    ipv4: &Option<String>,
    ipv6: &Option<String>,
) -> Result<Option<IpAddr>, String> {
    if let Some(address) = ipv4 {
        address
            .parse::<IpAddr>()
            .map(Some)
            .map_err(|_| format!("invalid IPv4 address: '{address}'"))
    } else if let Some(address) = ipv6 {
        address
            .parse::<IpAddr>()
            .map(Some)
            .map_err(|_| format!("invalid IPv6 address: '{address}'"))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_valid_inputs() {
        assert_eq!(OutputFormat::from("csv".to_string()), Ok(OutputFormat::Csv));
        assert_eq!(OutputFormat::from("CSV".to_string()), Ok(OutputFormat::Csv));
        assert_eq!(
            OutputFormat::from("json".to_string()),
            Ok(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from("JSON".to_string()),
            Ok(OutputFormat::Json)
        );
        assert_eq!(
            OutputFormat::from("json-pretty".to_string()),
            Ok(OutputFormat::JsonPretty)
        );
        assert_eq!(
            OutputFormat::from("json_pretty".to_string()),
            Ok(OutputFormat::JsonPretty)
        );
        assert_eq!(
            OutputFormat::from("JSON-PRETTY".to_string()),
            Ok(OutputFormat::JsonPretty)
        );
        assert_eq!(
            OutputFormat::from("stdout".to_string()),
            Ok(OutputFormat::StdOut)
        );
        assert_eq!(
            OutputFormat::from("STDOUT".to_string()),
            Ok(OutputFormat::StdOut)
        );
    }

    #[test]
    fn test_output_format_from_invalid_inputs() {
        assert!(OutputFormat::from("invalid".to_string()).is_err());
        assert!(OutputFormat::from("xml".to_string()).is_err());
        assert!(OutputFormat::from("".to_string()).is_err());
        assert!(OutputFormat::from("json_invalid".to_string()).is_err());

        let error_msg = OutputFormat::from("invalid".to_string()).unwrap_err();
        assert_eq!(
            error_msg,
            "Value needs to be one of csv, json or json-pretty"
        );
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(format!("{}", OutputFormat::Csv), "Csv");
        assert_eq!(format!("{}", OutputFormat::Json), "Json");
        assert_eq!(format!("{}", OutputFormat::JsonPretty), "JsonPretty");
        assert_eq!(format!("{}", OutputFormat::StdOut), "StdOut");
        assert_eq!(format!("{}", OutputFormat::None), "None");
    }

    #[test]
    fn test_parse_ipv4_address_accepts_valid_addresses() {
        assert_eq!(parse_ipv4_address("0.0.0.0"), Ok("0.0.0.0".to_string()));
        assert_eq!(
            parse_ipv4_address("192.168.1.5"),
            Ok("192.168.1.5".to_string())
        );
    }

    #[test]
    fn test_parse_ipv4_address_rejects_invalid_addresses() {
        assert!(parse_ipv4_address("not-an-ip").is_err());
        assert!(parse_ipv4_address("999.1.1.1").is_err());
        // IPv6 addresses belong to --ipv6, not --ipv4.
        assert!(parse_ipv4_address("::1").is_err());
    }

    #[test]
    fn test_parse_ipv6_address_accepts_valid_addresses() {
        assert_eq!(parse_ipv6_address("::"), Ok("::".to_string()));
        assert_eq!(parse_ipv6_address("fe80::1"), Ok("fe80::1".to_string()));
    }

    #[test]
    fn test_parse_ipv6_address_rejects_invalid_addresses() {
        assert!(parse_ipv6_address("not-an-ip").is_err());
        // IPv4 addresses belong to --ipv4, not --ipv6.
        assert!(parse_ipv6_address("192.168.1.5").is_err());
    }

    #[test]
    fn test_parse_bound_address() {
        assert_eq!(parse_bound_address(&None, &None), Ok(None));
        assert_eq!(
            parse_bound_address(&Some("127.0.0.1".to_string()), &None),
            Ok(Some("127.0.0.1".parse().unwrap()))
        );
        assert_eq!(
            parse_bound_address(&None, &Some("fe80::1".to_string())),
            Ok(Some("fe80::1".parse().unwrap()))
        );
        assert!(parse_bound_address(&Some("not-an-ip".to_string()), &None).is_err());
        assert!(parse_bound_address(&None, &Some("not-an-ip".to_string())).is_err());
    }

    #[test]
    fn test_cli_options_should_download() {
        let mut options = SpeedTestCLIOptions {
            nr_tests: 10,
            nr_latency_tests: 25,
            max_payload_size: speedtest::PayloadSize::M25,
            output_format: OutputFormat::StdOut,
            verbose: false,
            ipv4: None,
            ipv6: None,
            disable_dynamic_max_payload_size: false,
            download_only: false,
            upload_only: false,
            completion: None,
        };

        // Default: both download and upload
        assert!(options.should_download());
        assert!(options.should_upload());

        // Download only
        options.download_only = true;
        assert!(options.should_download());
        assert!(!options.should_upload());

        // Upload only
        options.download_only = false;
        options.upload_only = true;
        assert!(!options.should_download());
        assert!(options.should_upload());
    }

    #[test]
    fn test_cli_options_should_upload() {
        let mut options = SpeedTestCLIOptions {
            nr_tests: 10,
            nr_latency_tests: 25,
            max_payload_size: speedtest::PayloadSize::M25,
            output_format: OutputFormat::StdOut,
            verbose: false,
            ipv4: None,
            ipv6: None,
            disable_dynamic_max_payload_size: false,
            download_only: false,
            upload_only: false,
            completion: None,
        };

        // Default: both download and upload
        assert!(options.should_upload());
        assert!(options.should_download());

        // Upload only
        options.upload_only = true;
        assert!(options.should_upload());
        assert!(!options.should_download());

        // Download only
        options.upload_only = false;
        options.download_only = true;
        assert!(!options.should_upload());
        assert!(options.should_download());
    }
}
