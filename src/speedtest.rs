use crate::measurements::format_bytes;
use crate::measurements::log_measurements;
use crate::measurements::Measurement;
use crate::measurements::LatencyMeasurement;
use crate::progress::print_progress;
use crate::OutputFormat;
use crate::SpeedTestCLIOptions;
use log;
use regex::Regex;
use reqwest::{blocking::Client, StatusCode};
use serde::Serialize;
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";

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

pub struct Metadata {
    city: String,
    country: String,
    ip: String,
    asn: String,
    colo: String,
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "City: {}\nCountry: {}\nIp: {}\nAsn: {}\nColo: {}",
            self.city, self.country, self.ip, self.asn, self.colo
        )
    }
}

pub fn speed_test(client: Client, options: SpeedTestCLIOptions) -> Vec<Measurement> {
    let metadata = fetch_metadata(&client);
    if options.output_format == OutputFormat::StdOut {
        println!("{metadata}");
    }
    let (latency_measurements, avg_latency) = run_latency_test(&client, options.nr_latency_tests, options.output_format);
    let latency_measurement = if !latency_measurements.is_empty() {
        Some(LatencyMeasurement {
            avg_latency_ms: avg_latency,
            min_latency_ms: latency_measurements.iter().copied().fold(f64::INFINITY, f64::min),
            max_latency_ms: latency_measurements.iter().copied().fold(f64::NEG_INFINITY, f64::max),
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
    );
    measurements
}

pub fn run_latency_test(
    client: &Client,
    nr_latency_tests: u32,
    output_format: OutputFormat,
) -> (Vec<f64>, f64) {
    let mut measurements: Vec<f64> = Vec::new();
    for i in 0..=nr_latency_tests {
        if output_format == OutputFormat::StdOut {
            print_progress("latency test", i, nr_latency_tests);
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
    let response = req_builder.send().expect("failed to get response");
    let _status_code = response.status();
    let duration = start.elapsed().as_secs_f64() * 1_000.0;

    let re = Regex::new(r"cfRequestDuration;dur=([\d.]+)").unwrap();
    let cf_req_duration: f64 = re
        .captures(
            response
                .headers()
                .get("Server-Timing")
                .expect("No Server-Timing in response header")
                .to_str()
                .unwrap(),
        )
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .unwrap();
    let mut req_latency = duration - cf_req_duration;
    if req_latency < 0.0 {
        // TODO investigate negative latency values
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
    let (status_code, mbits, duration) = {
        let start = Instant::now();
        let response = req_builder.send().expect("failed to get response");
        let status_code = response.status();
        let duration = start.elapsed();
        let mbits = (payload_size_bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
        (status_code, mbits, duration)
    };
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
        let response = req_builder.send().expect("failed to get response");
        let status_code = response.status();
        let _res_bytes = response.bytes();
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

pub fn fetch_metadata(client: &Client) -> Metadata {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, 0);
    let headers = client
        .get(url)
        .send()
        .expect("failed to get response")
        .headers()
        .to_owned();
    Metadata {
        city: extract_header_value(&headers, "cf-meta-city", "City N/A"),
        country: extract_header_value(&headers, "cf-meta-country", "Country N/A"),
        ip: extract_header_value(&headers, "cf-meta-ip", "IP N/A"),
        asn: extract_header_value(&headers, "cf-meta-asn", "ASN N/A"),
        colo: extract_header_value(&headers, "cf-meta-colo", "Colo N/A"),
    }
}

fn extract_header_value(
    headers: &reqwest::header::HeaderMap,
    header_name: &str,
    na_value: &str,
) -> String {
    headers
        .get(header_name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(na_value)
        .to_owned()
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
        // The Display implementation uses format_bytes from measurements module
        // We'll test the basic functionality
        let size = PayloadSize::K100;
        let display_str = format!("{}", size);
        assert!(!display_str.is_empty());
    }
}
