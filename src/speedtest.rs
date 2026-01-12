use crate::measurements::format_bytes;
use crate::measurements::log_measurements;
use crate::measurements::LatencyMeasurement;
use crate::measurements::Measurement;
use crate::progress::print_progress;
use crate::OutputFormat;
use crate::SpeedTestCLIOptions;
use log;
use regex::Regex;
use reqwest::{blocking::Client, StatusCode};
use serde::Serialize;
use std::{
    fmt::Display,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";
static WARNED_NEGATIVE_LATENCY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Hash, Serialize, Eq, PartialEq)]
pub enum TestType {
    Download,
    Upload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayloadSize {
    K100 = 100_000,
    M1 = 1_000_000,
    M10 = 10_000_000,
    M25 = 25_000_000,
    M100 = 100_000_000,
}

impl Display for PayloadSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format_bytes(self.clone() as usize))
    }
}

impl PayloadSize {
    pub fn from(payload_string: String) -> Result<Self, String> {
        match payload_string.to_lowercase().as_str() {
            "100_000" | "100000" | "100k" | "100kb" => Ok(Self::K100),
            "1_000_000" | "1000000" | "1m" | "1mb" => Ok(Self::M1),
            "10_000_000" | "10000000" | "10m" | "10mb" => Ok(Self::M10),
            "25_000_000" | "25000000" | "25m" | "25mb" => Ok(Self::M25),
            "100_000_000" | "100000000" | "100m" | "100mb" => Ok(Self::M100),
            _ => Err("Value needs to be one of 100k, 1m, 10m, 25m or 100m".to_string()),
        }
    }

    pub fn sizes_from_max(max_payload_size: PayloadSize) -> Vec<usize> {
        log::debug!("getting payload iterations for max_payload_size {max_payload_size:?}");
        let payload_bytes: Vec<usize> =
            vec![100_000, 1_000_000, 10_000_000, 25_000_000, 100_000_000];
        match max_payload_size {
            PayloadSize::K100 => payload_bytes[0..1].to_vec(),
            PayloadSize::M1 => payload_bytes[0..2].to_vec(),
            PayloadSize::M10 => payload_bytes[0..3].to_vec(),
            PayloadSize::M25 => payload_bytes[0..4].to_vec(),
            PayloadSize::M100 => payload_bytes[0..5].to_vec(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Metadata {
    pub country: String,
    pub ip: String,
    pub colo: String,
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Country: {}\nIp: {}\nColo: {}",
            self.country, self.ip, self.colo
        )
    }
}

pub fn speed_test(client: Client, options: SpeedTestCLIOptions) -> Vec<Measurement> {
    let metadata = match fetch_metadata(&client) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error fetching metadata: {e}");
            std::process::exit(1);
        }
    };
    if options.output_format == OutputFormat::StdOut {
        println!("{metadata}");
    }
    let (latency_measurements, avg_latency) =
        run_latency_test(&client, options.nr_latency_tests, options.output_format);
    let latency_measurement = if !latency_measurements.is_empty() {
        Some(LatencyMeasurement {
            avg_latency_ms: avg_latency,
            min_latency_ms: latency_measurements
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min),
            max_latency_ms: latency_measurements
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
            latency_measurements,
        })
    } else {
        None
    };

    let payload_sizes = PayloadSize::sizes_from_max(options.max_payload_size.clone());
    let mut measurements = Vec::new();

    if options.should_download() {
        measurements.extend(run_tests(
            &client,
            test_download,
            TestType::Download,
            payload_sizes.clone(),
            options.nr_tests,
            options.output_format,
            options.disable_dynamic_max_payload_size,
        ));
    }

    if options.should_upload() {
        measurements.extend(run_tests(
            &client,
            test_upload,
            TestType::Upload,
            payload_sizes.clone(),
            options.nr_tests,
            options.output_format,
            options.disable_dynamic_max_payload_size,
        ));
    }

    log_measurements(
        &measurements,
        latency_measurement.as_ref(),
        payload_sizes,
        options.verbose,
        options.output_format,
        Some(&metadata),
    );
    measurements
}

pub fn run_latency_test(
    client: &Client,
    nr_latency_tests: u32,
    output_format: OutputFormat,
) -> (Vec<f64>, f64) {
    let mut measurements: Vec<f64> = Vec::new();
    for i in 0..nr_latency_tests {
        if output_format == OutputFormat::StdOut {
            print_progress("latency test", i + 1, nr_latency_tests);
        }
        let latency = test_latency(client);
        measurements.push(latency);
    }
    let avg_latency = measurements.iter().sum::<f64>() / measurements.len() as f64;

    if output_format == OutputFormat::StdOut {
        println!(
            "\nAvg GET request latency {avg_latency:.2} ms (RTT excluding server processing time)\n"
        );
    }
    (measurements, avg_latency)
}

pub fn test_latency(client: &Client) -> f64 {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, 0);
    let req_builder = client.get(url);

    let start = Instant::now();
    let mut response = req_builder.send().expect("failed to get response");
    let _status_code = response.status();
    // Drain body to complete the request; ignore errors.
    let _ = std::io::copy(&mut response, &mut std::io::sink());
    let total_ms = start.elapsed().as_secs_f64() * 1_000.0;

    let re = Regex::new(r"cfRequestDuration;dur=([\d.]+)").unwrap();
    let server_timing = response
        .headers()
        .get("Server-Timing")
        .expect("No Server-Timing in response header")
        .to_str()
        .unwrap();
    let cf_req_duration: f64 = re
        .captures(server_timing)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let mut req_latency = total_ms - cf_req_duration;
    log::debug!(
        "latency debug: total_ms={total_ms:.3} cf_req_duration_ms={cf_req_duration:.3} req_latency_total={req_latency:.3} server_timing={server_timing}"
    );
    if req_latency < 0.0 {
        if !WARNED_NEGATIVE_LATENCY.swap(true, Ordering::Relaxed) {
            log::warn!(
                "negative latency after server timing subtraction; clamping to 0.0 (total_ms={total_ms:.3} cf_req_duration_ms={cf_req_duration:.3})"
            );
        }
        req_latency = 0.0
    }
    req_latency
}

const TIME_THRESHOLD: Duration = Duration::from_secs(5);

pub fn run_tests(
    client: &Client,
    test_fn: fn(&Client, usize, OutputFormat) -> f64,
    test_type: TestType,
    payload_sizes: Vec<usize>,
    nr_tests: u32,
    output_format: OutputFormat,
    disable_dynamic_max_payload_size: bool,
) -> Vec<Measurement> {
    let mut measurements: Vec<Measurement> = Vec::new();
    for payload_size in payload_sizes {
        log::debug!("running tests for payload_size {payload_size}");
        let start = Instant::now();
        for i in 0..nr_tests {
            if output_format == OutputFormat::StdOut {
                print_progress(
                    &format!("{:?} {:<5}", test_type, format_bytes(payload_size)),
                    i,
                    nr_tests,
                );
            }
            let mbit = test_fn(client, payload_size, output_format);
            measurements.push(Measurement {
                test_type,
                payload_size,
                mbit,
            });
        }
        if output_format == OutputFormat::StdOut {
            print_progress(
                &format!("{:?} {:<5}", test_type, format_bytes(payload_size)),
                nr_tests,
                nr_tests,
            );
            println!()
        }
        let duration = start.elapsed();

        // only check TIME_THRESHOLD if dynamic max payload sizing is not disabled
        if !disable_dynamic_max_payload_size && duration > TIME_THRESHOLD {
            log::info!("Exceeded threshold");
            break;
        }
    }
    measurements
}

pub fn test_upload(client: &Client, payload_size_bytes: usize, output_format: OutputFormat) -> f64 {
    let url = &format!("{BASE_URL}/{UPLOAD_URL}");
    let payload: Vec<u8> = vec![1; payload_size_bytes];
    let req_builder = client.post(url).body(payload);
    let (mut response, status_code, mbits, duration) = {
        let start = Instant::now();
        let response = req_builder.send().expect("failed to get response");
        let status_code = response.status();
        let duration = start.elapsed();
        let mbits = (payload_size_bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
        (response, status_code, mbits, duration)
    };
    // Drain response after timing so we don't skew upload measurement.
    let _ = std::io::copy(&mut response, &mut std::io::sink());
    if output_format == OutputFormat::StdOut {
        print_current_speed(mbits, duration, status_code, payload_size_bytes);
    }
    mbits
}

pub fn test_download(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
) -> f64 {
    let url = &format!("{BASE_URL}/{DOWNLOAD_URL}{payload_size_bytes}");
    let req_builder = client.get(url);
    let (status_code, mbits, duration) = {
        let start = Instant::now();
        let mut response = req_builder.send().expect("failed to get response");
        let status_code = response.status();
        // Stream the body to avoid buffering the full payload in memory.
        let _ = std::io::copy(&mut response, &mut std::io::sink());
        let duration = start.elapsed();
        let mbits = (payload_size_bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
        (status_code, mbits, duration)
    };
    if output_format == OutputFormat::StdOut {
        print_current_speed(mbits, duration, status_code, payload_size_bytes);
    }
    mbits
}

fn print_current_speed(
    mbits: f64,
    duration: Duration,
    status_code: StatusCode,
    payload_size_bytes: usize,
) {
    print!(
        "  {:>6.2} mbit/s | {:>5} in {:>4}ms -> status: {}  ",
        mbits,
        format_bytes(payload_size_bytes),
        duration.as_millis(),
        status_code
    );
}

pub fn fetch_metadata(client: &Client) -> Result<Metadata, reqwest::Error> {
    const TRACE_URL: &str = "https://speed.cloudflare.com/cdn-cgi/trace";

    let response = client.get(TRACE_URL).send()?;
    let body = response.text()?;

    // Parse key=value pairs from response body
    let trace_data = parse_trace_response(&body);

    Ok(Metadata {
        country: trace_data
            .get("loc")
            .unwrap_or(&"N/A".to_string())
            .to_owned(),
        ip: trace_data
            .get("ip")
            .unwrap_or(&"N/A".to_string())
            .to_owned(),
        colo: trace_data
            .get("colo")
            .unwrap_or(&"N/A".to_string())
            .to_owned(),
    })
}

/// Parses the Cloudflare trace response body into a key-value map
///
/// The trace endpoint returns plain text in the format:
/// key1=value1
/// key2=value2
///
/// This function splits the response by newlines and then by '=' to create a HashMap
fn parse_trace_response(body: &str) -> std::collections::HashMap<String, String> {
    body.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
            } else {
                log::debug!("Skipping malformed trace line: {}", line);
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_size_from_valid_inputs() {
        // Test 100K variants
        assert_eq!(PayloadSize::from("100k".to_string()), Ok(PayloadSize::K100));
        assert_eq!(PayloadSize::from("100K".to_string()), Ok(PayloadSize::K100));
        assert_eq!(
            PayloadSize::from("100kb".to_string()),
            Ok(PayloadSize::K100)
        );
        assert_eq!(
            PayloadSize::from("100KB".to_string()),
            Ok(PayloadSize::K100)
        );
        assert_eq!(
            PayloadSize::from("100000".to_string()),
            Ok(PayloadSize::K100)
        );
        assert_eq!(
            PayloadSize::from("100_000".to_string()),
            Ok(PayloadSize::K100)
        );

        // Test 1M variants
        assert_eq!(PayloadSize::from("1m".to_string()), Ok(PayloadSize::M1));
        assert_eq!(PayloadSize::from("1M".to_string()), Ok(PayloadSize::M1));
        assert_eq!(PayloadSize::from("1mb".to_string()), Ok(PayloadSize::M1));
        assert_eq!(PayloadSize::from("1MB".to_string()), Ok(PayloadSize::M1));
        assert_eq!(
            PayloadSize::from("1000000".to_string()),
            Ok(PayloadSize::M1)
        );
        assert_eq!(
            PayloadSize::from("1_000_000".to_string()),
            Ok(PayloadSize::M1)
        );

        // Test 10M variants
        assert_eq!(PayloadSize::from("10m".to_string()), Ok(PayloadSize::M10));
        assert_eq!(PayloadSize::from("10M".to_string()), Ok(PayloadSize::M10));
        assert_eq!(PayloadSize::from("10mb".to_string()), Ok(PayloadSize::M10));
        assert_eq!(PayloadSize::from("10MB".to_string()), Ok(PayloadSize::M10));
        assert_eq!(
            PayloadSize::from("10000000".to_string()),
            Ok(PayloadSize::M10)
        );
        assert_eq!(
            PayloadSize::from("10_000_000".to_string()),
            Ok(PayloadSize::M10)
        );

        // Test 25M variants
        assert_eq!(PayloadSize::from("25m".to_string()), Ok(PayloadSize::M25));
        assert_eq!(PayloadSize::from("25M".to_string()), Ok(PayloadSize::M25));
        assert_eq!(PayloadSize::from("25mb".to_string()), Ok(PayloadSize::M25));
        assert_eq!(PayloadSize::from("25MB".to_string()), Ok(PayloadSize::M25));
        assert_eq!(
            PayloadSize::from("25000000".to_string()),
            Ok(PayloadSize::M25)
        );
        assert_eq!(
            PayloadSize::from("25_000_000".to_string()),
            Ok(PayloadSize::M25)
        );

        // Test 100M variants
        assert_eq!(PayloadSize::from("100m".to_string()), Ok(PayloadSize::M100));
        assert_eq!(PayloadSize::from("100M".to_string()), Ok(PayloadSize::M100));
        assert_eq!(
            PayloadSize::from("100mb".to_string()),
            Ok(PayloadSize::M100)
        );
        assert_eq!(
            PayloadSize::from("100MB".to_string()),
            Ok(PayloadSize::M100)
        );
        assert_eq!(
            PayloadSize::from("100000000".to_string()),
            Ok(PayloadSize::M100)
        );
        assert_eq!(
            PayloadSize::from("100_000_000".to_string()),
            Ok(PayloadSize::M100)
        );
    }

    #[test]
    fn test_payload_size_from_invalid_inputs() {
        assert!(PayloadSize::from("invalid".to_string()).is_err());
        assert!(PayloadSize::from("50m".to_string()).is_err());
        assert!(PayloadSize::from("200k".to_string()).is_err());
        assert!(PayloadSize::from("".to_string()).is_err());
        assert!(PayloadSize::from("1g".to_string()).is_err());

        let error_msg = PayloadSize::from("invalid".to_string()).unwrap_err();
        assert_eq!(
            error_msg,
            "Value needs to be one of 100k, 1m, 10m, 25m or 100m"
        );
    }

    #[test]
    fn test_payload_size_values() {
        assert_eq!(PayloadSize::K100 as usize, 100_000);
        assert_eq!(PayloadSize::M1 as usize, 1_000_000);
        assert_eq!(PayloadSize::M10 as usize, 10_000_000);
        assert_eq!(PayloadSize::M25 as usize, 25_000_000);
        assert_eq!(PayloadSize::M100 as usize, 100_000_000);
    }

    #[test]
    fn test_payload_size_sizes_from_max() {
        assert_eq!(
            PayloadSize::sizes_from_max(PayloadSize::K100),
            vec![100_000]
        );
        assert_eq!(
            PayloadSize::sizes_from_max(PayloadSize::M1),
            vec![100_000, 1_000_000]
        );
        assert_eq!(
            PayloadSize::sizes_from_max(PayloadSize::M10),
            vec![100_000, 1_000_000, 10_000_000]
        );
        assert_eq!(
            PayloadSize::sizes_from_max(PayloadSize::M25),
            vec![100_000, 1_000_000, 10_000_000, 25_000_000]
        );
        assert_eq!(
            PayloadSize::sizes_from_max(PayloadSize::M100),
            vec![100_000, 1_000_000, 10_000_000, 25_000_000, 100_000_000]
        );
    }

    #[test]
    fn test_payload_size_display() {
        let size = PayloadSize::K100;
        let display_str = format!("{size}");
        assert!(!display_str.is_empty());
    }

    #[test]
    fn test_fetch_metadata_ipv6_timeout_error() {
        use std::time::Duration;

        let client = reqwest::blocking::Client::builder()
            .local_address("::".parse::<std::net::IpAddr>().unwrap())
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();

        let result = fetch_metadata(&client);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_trace_response_valid() {
        let body = "ip=178.197.211.5\ncolo=ZRH\nloc=CH\nts=1768250090.213\n";
        let parsed = parse_trace_response(body);

        assert_eq!(parsed.get("ip"), Some(&"178.197.211.5".to_string()));
        assert_eq!(parsed.get("colo"), Some(&"ZRH".to_string()));
        assert_eq!(parsed.get("loc"), Some(&"CH".to_string()));
        assert_eq!(parsed.get("ts"), Some(&"1768250090.213".to_string()));
    }

    #[test]
    fn test_parse_trace_response_empty() {
        let body = "";
        let parsed = parse_trace_response(body);
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_trace_response_malformed_lines() {
        let body = "ip=178.197.211.5\nmalformed_line\ncolo=ZRH\n";
        let parsed = parse_trace_response(body);

        assert_eq!(parsed.get("ip"), Some(&"178.197.211.5".to_string()));
        assert_eq!(parsed.get("colo"), Some(&"ZRH".to_string()));
        assert_eq!(parsed.len(), 2); // malformed line should be skipped
    }

    #[test]
    fn test_parse_trace_response_with_equals_in_value() {
        let body = "key1=value1\nkey2=value=with=equals\n";
        let parsed = parse_trace_response(body);

        assert_eq!(parsed.get("key1"), Some(&"value1".to_string()));
        assert_eq!(parsed.get("key2"), Some(&"value=with=equals".to_string()));
    }

    #[test]
    #[ignore] // Remove #[ignore] to run in CI nightly pipeline
    fn test_fetch_metadata_integration() {
        // This test verifies that Cloudflare's trace endpoint returns the expected metadata fields.
        // If this test starts failing, it means Cloudflare changed their API again.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let result = fetch_metadata(&client);

        assert!(
            result.is_ok(),
            "Failed to fetch metadata: {:?}",
            result.err()
        );
        let metadata = result.unwrap();

        // These fields MUST be populated (not "N/A") for the API to be working correctly
        assert_ne!(metadata.ip, "N/A", "IP field should be populated");
        assert_ne!(
            metadata.colo, "N/A",
            "Colo field should be populated (CRITICAL: Cloudflare API may have changed)"
        );
        assert_ne!(
            metadata.country, "N/A",
            "Country field should be populated (CRITICAL: Cloudflare API may have changed)"
        );

        // Validate format: IP should be a valid IP address format
        assert!(
            metadata.ip.contains('.') || metadata.ip.contains(':'),
            "IP should be in valid format (IPv4 or IPv6): {}",
            metadata.ip
        );

        // Validate format: Colo should be 3 uppercase letters (IATA code)
        assert_eq!(
            metadata.colo.len(),
            3,
            "Colo should be 3-letter IATA code: {}",
            metadata.colo
        );
        assert!(
            metadata.colo.chars().all(|c| c.is_ascii_uppercase()),
            "Colo should be uppercase letters: {}",
            metadata.colo
        );

        // Validate format: Country should be 2 uppercase letters (ISO code)
        assert_eq!(
            metadata.country.len(),
            2,
            "Country should be 2-letter ISO code: {}",
            metadata.country
        );
        assert!(
            metadata.country.chars().all(|c| c.is_ascii_uppercase()),
            "Country should be uppercase letters: {}",
            metadata.country
        );

        eprintln!(
            "✓ Metadata integration test passed: ip={}, colo={}, country={}",
            metadata.ip, metadata.colo, metadata.country
        );
    }
}
