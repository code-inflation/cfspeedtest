use crate::measurements::format_bytes;
use crate::measurements::log_measurements;
use crate::measurements::Measurement;
use crate::measurements::PayloadAttemptStats;
use crate::progress::print_progress;
use crate::run::{
    interruptible_sleep, LatencyReport, LatencyStatus, MeasurementError, RunConfig, RunControl,
    RunStatus, SpeedTestReport,
};
use crate::stdout;
use crate::OutputFormat;
use crate::SpeedTestCLIOptions;
use jiff::Zoned;
use log;
use regex::Regex;
use reqwest::{blocking::Client, header::RETRY_AFTER, StatusCode};
use serde::Serialize;
use std::{
    fmt::Display,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
    time::{Duration, Instant, SystemTime},
};

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";
static RE_CF_REQUEST_DURATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"cfRequestDuration;dur=([\d.]+)").unwrap());
static RE_CFL4_RTT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]rtt=(\d+)").unwrap());
static WARNED_NO_HEADER: AtomicBool = AtomicBool::new(false);
static WARNED_UNKNOWN_HEADER: AtomicBool = AtomicBool::new(false);
const TIME_THRESHOLD: Duration = Duration::from_secs(5);
const MAX_ATTEMPT_FACTOR: u32 = 4;
const RETRY_BASE_BACKOFF: Duration = Duration::from_millis(250);
const RETRY_MAX_BACKOFF: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug)]
struct RetryRunOptions {
    nr_tests: u32,
    output_format: OutputFormat,
    disable_dynamic_max_payload_size: bool,
}

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

/// Compatibility wrapper. Returns successful throughput samples and never exits the process.
/// Use `speed_test_with_config` for explicit outcomes, latency, errors, and limits.
pub fn speed_test(client: Client, options: SpeedTestCLIOptions) -> Vec<Measurement> {
    speed_test_with_config(client, options, RunConfig::default()).measurements
}

/// Run with a single deadline and retry-wait budget shared by every stage.
/// Output follows `options.output_format`; the returned report is available in every format.
pub fn speed_test_with_config(
    client: Client,
    options: SpeedTestCLIOptions,
    config: RunConfig,
) -> SpeedTestReport {
    let mut control = RunControl::new(config);
    let base_url = control.config.base_url.trim_end_matches('/').to_string();
    let metadata = if let Some(timeout) = control.request_timeout() {
        match fetch_metadata_request(&client, &base_url, Some(timeout)) {
            Ok(metadata) => Some(metadata),
            Err(error) => {
                control.record("metadata", None, error);
                None
            }
        }
    } else {
        None
    };
    if options.output_format == OutputFormat::StdOut {
        if let Some(metadata) = &metadata {
            stdout::print_line(&metadata.to_string());
        }
    }
    let latency = run_latency_with_control(
        &client,
        options.nr_latency_tests,
        options.output_format,
        &mut control,
    );
    let payload_sizes = PayloadSize::sizes_from_max(options.max_payload_size.clone());
    let retry_options = RetryRunOptions {
        nr_tests: options.nr_tests,
        output_format: options.output_format,
        disable_dynamic_max_payload_size: options.disable_dynamic_max_payload_size,
    };
    let mut measurements = Vec::new();
    let mut payload_attempt_stats = Vec::new();
    let cancelled = control.config.cancelled.clone();
    for (enabled, test_type) in [
        (options.should_download(), TestType::Download),
        (options.should_upload(), TestType::Upload),
    ] {
        if enabled {
            let (samples, attempts) = run_tests_with_control(
                &client,
                test_type,
                payload_sizes.clone(),
                retry_options,
                &mut control,
                |delay| interruptible_sleep(delay, &cancelled),
            );
            measurements.extend(samples);
            payload_attempt_stats.extend(attempts);
        }
    }
    control.remaining();
    let status = if measurements.is_empty() {
        RunStatus::Failed
    } else if control.stop_reason.is_some()
        || matches!(
            latency.status,
            LatencyStatus::Failed | LatencyStatus::Partial
        )
        || payload_attempt_stats
            .iter()
            .any(|s| s.successes < s.target_successes)
    {
        RunStatus::Partial
    } else {
        RunStatus::Complete
    };
    let report = SpeedTestReport {
        status,
        stop_reason: control.stop_reason,
        metadata,
        latency,
        measurements,
        payload_attempt_stats,
        errors: control.errors,
    };
    log_measurements(
        &report,
        payload_sizes,
        options.verbose,
        options.output_format,
    );
    report
}

/// Compatibility wrapper. The average is NaN when there are no valid samples.
/// Use `run_latency_report` to distinguish disabled, partial, and failed measurements.
pub fn run_latency_test(
    client: &Client,
    nr_latency_tests: u32,
    output_format: OutputFormat,
) -> (Vec<f64>, f64) {
    let report = run_latency_report(
        client,
        nr_latency_tests,
        output_format,
        RunConfig::default(),
    );
    (
        report.latency_measurements,
        report.avg_latency_ms.unwrap_or(f64::NAN),
    )
}

pub fn run_latency_report(
    client: &Client,
    nr_latency_tests: u32,
    output_format: OutputFormat,
    config: RunConfig,
) -> LatencyReport {
    run_latency_with_control(
        client,
        nr_latency_tests,
        output_format,
        &mut RunControl::new(config),
    )
}

fn run_latency_with_control(
    client: &Client,
    nr_latency_tests: u32,
    output_format: OutputFormat,
    control: &mut RunControl,
) -> LatencyReport {
    let mut report = LatencyReport::new(nr_latency_tests);
    let base_url = control.config.base_url.trim_end_matches('/').to_string();
    for i in 0..nr_latency_tests {
        let Some(timeout) = control.request_timeout() else {
            break;
        };
        if output_format == OutputFormat::StdOut {
            print_progress("latency test", i + 1, nr_latency_tests);
        }
        report.attempts += 1;
        match test_latency_request(client, &base_url, Some(timeout)) {
            Ok(latency) => report.latency_measurements.push(latency),
            Err(error) => {
                control.record("latency", None, error.clone());
                report.errors.push(error);
            }
        }
    }
    report.finish();
    if output_format == OutputFormat::StdOut && nr_latency_tests > 0 {
        if let Some(avg) = report.avg_latency_ms {
            stdout::print_line(&format!(
                "\nAvg GET request latency {avg:.2} ms ({}/{} valid samples)\n",
                report.successes, report.target_samples
            ));
        } else {
            stdout::print_line(&format!(
                "\nAvg GET request latency N/A (0/{} valid samples)\n",
                report.target_samples
            ));
        }
    }
    report
}

// Parse latency from a Server-Timing header value. Supports the legacy
// cfRequestDuration format and the newer cfL4 format. Returns None if
// the header doesn't match either.
fn parse_latency_from_server_timing(header: &str, total_ms: f64) -> Option<f64> {
    // Legacy: cfRequestDuration;dur=<milliseconds>
    if let Some(caps) = RE_CF_REQUEST_DURATION.captures(header) {
        if let Some(dur_match) = caps.get(1) {
            if let Ok(server_duration) = dur_match.as_str().parse::<f64>() {
                let latency = total_ms - server_duration;
                return Some(if latency < 0.0 { 0.0 } else { latency });
            }
        }
    }

    // Current: cfL4;desc="?...&rtt=<microseconds>&..."
    // [?&] anchor prevents matching min_rtt= or rtt_var=
    if header.contains("cfL4") {
        if let Some(caps) = RE_CFL4_RTT.captures(header) {
            if let Some(rtt_match) = caps.get(1) {
                if let Ok(rtt_us) = rtt_match.as_str().parse::<f64>() {
                    return Some(rtt_us / 1_000.0);
                }
            }
        }
    }

    None
}

/// Measure one valid latency response, retaining the cause of a failed measurement.
pub fn try_test_latency(client: &Client) -> Result<f64, MeasurementError> {
    test_latency_request(client, BASE_URL, None)
}

fn test_latency_request(
    client: &Client,
    base_url: &str,
    timeout: Option<Duration>,
) -> Result<f64, MeasurementError> {
    let url = format!("{base_url}/{DOWNLOAD_URL}0");
    let mut request = client.get(url);
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }
    let start = Instant::now();
    let mut response = request.send()?.error_for_status()?;
    let status_code = response.status().as_u16();
    if status_code != 200 {
        return Err(MeasurementError {
            status_code: Some(status_code),
            reason: "unexpected latency response status".into(),
        });
    }
    let received =
        std::io::copy(&mut response, &mut std::io::sink()).map_err(|error| MeasurementError {
            status_code: Some(status_code),
            reason: format!("failed to read latency body: {error}"),
        })?;
    if received != 0 {
        return Err(MeasurementError {
            status_code: Some(status_code),
            reason: format!("expected empty latency body, received {received} bytes"),
        });
    }
    let total_ms = start.elapsed().as_secs_f64() * 1_000.0;
    let server_timing = response
        .headers()
        .get("Server-Timing")
        .and_then(|v| v.to_str().ok());
    if let Some(header) = server_timing {
        if let Some(latency) = parse_latency_from_server_timing(header, total_ms) {
            if latency.is_finite() && latency >= 0.0 {
                return Ok(latency);
            }
            return Err(MeasurementError {
                status_code: Some(status_code),
                reason: "non-finite latency in Server-Timing".into(),
            });
        }
        if !WARNED_UNKNOWN_HEADER.swap(true, Ordering::Relaxed) {
            log::warn!("Server-Timing header format not recognized, falling back to raw RTT");
        }
    } else if !WARNED_NO_HEADER.swap(true, Ordering::Relaxed) {
        log::warn!("No Server-Timing header in response, falling back to raw RTT");
    }
    Ok(total_ms)
}

/// Compatibility wrapper. Returns NaN on failure; prefer `try_test_latency`.
pub fn test_latency(client: &Client) -> f64 {
    try_test_latency(client).unwrap_or(f64::NAN)
}

#[derive(Debug)]
enum SampleOutcome {
    Success {
        mbits: f64,
        duration: Duration,
        status_code: StatusCode,
    },
    RetryableFailure {
        duration: Duration,
        status_code: Option<StatusCode>,
        retry_after: Option<Duration>,
        reason: String,
    },
    Failed {
        duration: Duration,
        status_code: Option<StatusCode>,
        reason: String,
    },
}

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
        log::debug!("running compatibility test loop for payload_size {payload_size}");
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
            if mbit.is_finite() {
                measurements.push(Measurement {
                    test_type,
                    payload_size,
                    mbit,
                });
            }
        }
        if output_format == OutputFormat::StdOut {
            print_progress(
                &format!("{:?} {:<5}", test_type, format_bytes(payload_size)),
                nr_tests,
                nr_tests,
            );
            stdout::print_line("");
        }
        if !disable_dynamic_max_payload_size && start.elapsed() > TIME_THRESHOLD {
            log::info!("Exceeded threshold");
            break;
        }
    }
    measurements
}

pub fn run_tests_with_retries(
    client: &Client,
    test_type: TestType,
    payload_sizes: Vec<usize>,
    nr_tests: u32,
    output_format: OutputFormat,
    disable_dynamic_max_payload_size: bool,
) -> (Vec<Measurement>, Vec<PayloadAttemptStats>) {
    let mut control = RunControl::new(RunConfig::default());
    let cancelled = control.config.cancelled.clone();
    run_tests_with_control(
        client,
        test_type,
        payload_sizes,
        RetryRunOptions {
            nr_tests,
            output_format,
            disable_dynamic_max_payload_size,
        },
        &mut control,
        |delay| interruptible_sleep(delay, &cancelled),
    )
}

#[cfg(test)]
fn run_tests_with_sleep<S: Fn(Duration)>(
    client: &Client,
    test_type: TestType,
    payload_sizes: Vec<usize>,
    options: RetryRunOptions,
    base_url: &str,
    sleep_fn: S,
) -> (Vec<Measurement>, Vec<PayloadAttemptStats>) {
    let mut control = RunControl::new(RunConfig {
        base_url: base_url.into(),
        ..RunConfig::default()
    });
    run_tests_with_control(
        client,
        test_type,
        payload_sizes,
        options,
        &mut control,
        sleep_fn,
    )
}

fn run_tests_with_control<S: Fn(Duration)>(
    client: &Client,
    test_type: TestType,
    payload_sizes: Vec<usize>,
    options: RetryRunOptions,
    control: &mut RunControl,
    sleep_fn: S,
) -> (Vec<Measurement>, Vec<PayloadAttemptStats>) {
    let base_url = control.config.base_url.trim_end_matches('/').to_string();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut payload_attempt_stats = Vec::new();

    for payload_size in payload_sizes {
        if control.remaining().is_none() {
            break;
        }
        let label = format!("{:?} {:<5}", test_type, format_bytes(payload_size));
        log::debug!("running tests for payload_size {payload_size}");
        let start = Instant::now();

        let mut attempts = 0;
        let mut successes = 0;
        let mut skipped = 0;
        let mut retry_streak = 0;
        let max_attempts = options
            .nr_tests
            .saturating_mul(MAX_ATTEMPT_FACTOR)
            .max(options.nr_tests);

        while successes < options.nr_tests && attempts < max_attempts {
            let Some(timeout) = control.request_timeout() else {
                break;
            };
            if options.output_format == OutputFormat::StdOut {
                print_progress(&label, successes, options.nr_tests);
            }

            attempts += 1;
            let sample_outcome = match test_type {
                TestType::Download => test_download_request(
                    client,
                    payload_size,
                    options.output_format,
                    &base_url,
                    Some(timeout),
                ),
                TestType::Upload => test_upload_request(
                    client,
                    payload_size,
                    options.output_format,
                    &base_url,
                    Some(timeout),
                ),
            };

            match sample_outcome {
                SampleOutcome::Success {
                    mbits,
                    duration,
                    status_code,
                } => {
                    log::debug!(
                        "{test_type:?} {} success: status={} duration={}ms throughput={mbits:.2} mbit/s",
                        format_bytes(payload_size),
                        status_code,
                        duration.as_millis(),
                    );
                    successes += 1;
                    retry_streak = 0;
                    measurements.push(Measurement {
                        test_type,
                        payload_size,
                        mbit: mbits,
                    });
                }
                SampleOutcome::RetryableFailure {
                    duration,
                    status_code,
                    retry_after,
                    reason,
                } => {
                    skipped += 1;
                    control.record(
                        &format!("{test_type:?}").to_lowercase(),
                        Some(payload_size),
                        MeasurementError {
                            status_code: status_code.map(|s| s.as_u16()),
                            reason: reason.clone(),
                        },
                    );
                    retry_streak += 1;
                    if attempts < max_attempts {
                        let delay = compute_retry_delay(retry_streak, retry_after);
                        if !control.reserve_retry(delay) {
                            break;
                        }
                        let status = status_code
                            .map(|code| code.to_string())
                            .unwrap_or_else(|| "transport error".to_string());
                        log::warn!(
                            "{test_type:?} {} failed ({status}) after {}ms: {reason}. retrying in {}ms ({attempts}/{max_attempts})",
                            format_bytes(payload_size),
                            duration.as_millis(),
                            delay.as_millis(),
                        );
                        if options.output_format == OutputFormat::StdOut {
                            print_retry_notice(delay, attempts, max_attempts);
                        }
                        sleep_fn(delay);
                    }
                }
                SampleOutcome::Failed {
                    duration,
                    status_code,
                    reason,
                } => {
                    skipped += 1;
                    control.record(
                        &format!("{test_type:?}").to_lowercase(),
                        Some(payload_size),
                        MeasurementError {
                            status_code: status_code.map(|s| s.as_u16()),
                            reason: reason.clone(),
                        },
                    );
                    let status = status_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "transport error".to_string());
                    log::warn!(
                        "{test_type:?} {} failed ({status}) after {}ms: {reason}. aborting this payload",
                        format_bytes(payload_size),
                        duration.as_millis(),
                    );
                    break;
                }
            }
        }

        if options.output_format == OutputFormat::StdOut {
            print_progress(&label, successes, options.nr_tests);
            stdout::print_line("");
        }

        payload_attempt_stats.push(PayloadAttemptStats {
            test_type,
            payload_size,
            attempts,
            successes,
            skipped,
            target_successes: options.nr_tests,
        });

        if successes < options.nr_tests {
            log::warn!(
                "{test_type:?} {} collected {successes}/{} successful samples after {attempts} attempts",
                format_bytes(payload_size),
                options.nr_tests,
            );
        }

        let duration = start.elapsed();
        if !options.disable_dynamic_max_payload_size && duration > TIME_THRESHOLD {
            log::info!("Exceeded threshold");
            break;
        }
    }

    (measurements, payload_attempt_stats)
}

pub fn test_upload(client: &Client, payload_size_bytes: usize, output_format: OutputFormat) -> f64 {
    match test_upload_with_base_url(client, payload_size_bytes, output_format, BASE_URL) {
        SampleOutcome::Success { mbits, .. } => mbits,
        SampleOutcome::RetryableFailure { .. } | SampleOutcome::Failed { .. } => f64::NAN,
    }
}

pub fn test_download(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
) -> f64 {
    match test_download_with_base_url(client, payload_size_bytes, output_format, BASE_URL) {
        SampleOutcome::Success { mbits, .. } => mbits,
        SampleOutcome::RetryableFailure { .. } | SampleOutcome::Failed { .. } => f64::NAN,
    }
}

fn test_upload_with_base_url(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
    base_url: &str,
) -> SampleOutcome {
    test_upload_request(client, payload_size_bytes, output_format, base_url, None)
}

fn test_upload_request(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
    base_url: &str,
    timeout: Option<Duration>,
) -> SampleOutcome {
    let url = format!("{base_url}/{UPLOAD_URL}");
    let payload: Vec<u8> = vec![1; payload_size_bytes];
    let mut req_builder = client.post(&url).body(payload);
    if let Some(timeout) = timeout {
        req_builder = req_builder.timeout(timeout);
    }

    let start = Instant::now();
    let mut response = match req_builder.send() {
        Ok(response) => response,
        Err(error) => {
            let duration = start.elapsed();
            if output_format == OutputFormat::StdOut {
                print_transport_failure(duration, payload_size_bytes, &error);
            }
            if error.is_timeout() {
                return SampleOutcome::RetryableFailure {
                    duration,
                    status_code: None,
                    retry_after: None,
                    reason: error.to_string(),
                };
            }
            return SampleOutcome::Failed {
                duration,
                status_code: None,
                reason: error.to_string(),
            };
        }
    };

    let status_code = response.status();
    let retry_after = parse_retry_after(response.headers().get(RETRY_AFTER));
    // Measure upload duration once response headers are available.
    let duration = start.elapsed();
    // Validate completion after timing so response download time does not skew upload speed.
    let body_result = std::io::copy(&mut response, &mut std::io::sink());
    if !status_code.is_success() {
        if output_format == OutputFormat::StdOut {
            print_skipped_sample(duration, status_code, payload_size_bytes);
        }
        return if is_retryable_status(status_code) {
            SampleOutcome::RetryableFailure {
                duration,
                status_code: Some(status_code),
                retry_after,
                reason: "retryable HTTP status".to_string(),
            }
        } else {
            SampleOutcome::Failed {
                duration,
                status_code: Some(status_code),
                reason: "non-retryable HTTP status".to_string(),
            }
        };
    }

    if let Err(error) = body_result {
        return SampleOutcome::RetryableFailure {
            duration,
            status_code: Some(status_code),
            retry_after: None,
            reason: format!("failed to read upload response: {error}"),
        };
    }
    let mbits = (payload_size_bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
    if !mbits.is_finite() || mbits <= 0.0 {
        return SampleOutcome::Failed {
            duration,
            status_code: Some(status_code),
            reason: "invalid upload throughput".into(),
        };
    }
    if output_format == OutputFormat::StdOut {
        print_current_speed(mbits, duration, payload_size_bytes);
    }
    SampleOutcome::Success {
        mbits,
        duration,
        status_code,
    }
}

fn test_download_with_base_url(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
    base_url: &str,
) -> SampleOutcome {
    test_download_request(client, payload_size_bytes, output_format, base_url, None)
}

fn test_download_request(
    client: &Client,
    payload_size_bytes: usize,
    output_format: OutputFormat,
    base_url: &str,
    timeout: Option<Duration>,
) -> SampleOutcome {
    let url = format!("{base_url}/{DOWNLOAD_URL}{payload_size_bytes}");
    let mut req_builder = client.get(&url);
    if let Some(timeout) = timeout {
        req_builder = req_builder.timeout(timeout);
    }

    let start = Instant::now();
    let mut response = match req_builder.send() {
        Ok(response) => response,
        Err(error) => {
            let duration = start.elapsed();
            if output_format == OutputFormat::StdOut {
                print_transport_failure(duration, payload_size_bytes, &error);
            }
            if error.is_timeout() {
                return SampleOutcome::RetryableFailure {
                    duration,
                    status_code: None,
                    retry_after: None,
                    reason: error.to_string(),
                };
            }
            return SampleOutcome::Failed {
                duration,
                status_code: None,
                reason: error.to_string(),
            };
        }
    };

    let status_code = response.status();
    // Retain body errors and byte counts: a successful status alone is not a sample.
    let body_result = std::io::copy(&mut response, &mut std::io::sink());
    let duration = start.elapsed();
    if !status_code.is_success() {
        if output_format == OutputFormat::StdOut {
            print_skipped_sample(duration, status_code, payload_size_bytes);
        }
        let retry_after = parse_retry_after(response.headers().get(RETRY_AFTER));
        return if is_retryable_status(status_code) {
            SampleOutcome::RetryableFailure {
                duration,
                status_code: Some(status_code),
                retry_after,
                reason: "retryable HTTP status".to_string(),
            }
        } else {
            SampleOutcome::Failed {
                duration,
                status_code: Some(status_code),
                reason: "non-retryable HTTP status".to_string(),
            }
        };
    }

    let received = match body_result {
        Ok(received) => received,
        Err(error) => {
            return SampleOutcome::RetryableFailure {
                duration,
                status_code: Some(status_code),
                retry_after: None,
                reason: format!("failed to read download body: {error}"),
            }
        }
    };
    if received != payload_size_bytes as u64 {
        return SampleOutcome::Failed {
            duration,
            status_code: Some(status_code),
            reason: format!(
                "incorrect download size: expected {payload_size_bytes} bytes, received {received}"
            ),
        };
    }
    let mbits = (received as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
    if !mbits.is_finite() || mbits <= 0.0 {
        return SampleOutcome::Failed {
            duration,
            status_code: Some(status_code),
            reason: "invalid download throughput".to_string(),
        };
    }
    if output_format == OutputFormat::StdOut {
        print_current_speed(mbits, duration, payload_size_bytes);
    }
    SampleOutcome::Success {
        mbits,
        duration,
        status_code,
    }
}

fn is_retryable_status(status_code: StatusCode) -> bool {
    matches!(
        status_code.as_u16(),
        408 | 425 | 429 | 500 | 502 | 503 | 504
    )
}

fn parse_retry_after(retry_after: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    parse_retry_after_at(retry_after, SystemTime::now())
}

fn parse_retry_after_at(
    retry_after: Option<&reqwest::header::HeaderValue>,
    now: SystemTime,
) -> Option<Duration> {
    let value = retry_after?.to_str().ok()?.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        // Overflow must not turn an enormous server delay into a short retry.
        return Some(Duration::from_secs(
            value.parse::<u64>().unwrap_or(u64::MAX),
        ));
    }
    httpdate::parse_http_date(value)
        .ok()
        .map(|time| time.duration_since(now).unwrap_or(Duration::ZERO))
}

fn compute_retry_delay(retry_count: u32, retry_after: Option<Duration>) -> Duration {
    if let Some(delay) = retry_after {
        return delay;
    }

    let exponent = retry_count.saturating_sub(1).min(4);
    let base_delay_ms = RETRY_BASE_BACKOFF.as_millis() as u64;
    let capped_delay_ms = RETRY_MAX_BACKOFF.as_millis() as u64;
    let delay_ms = base_delay_ms
        .saturating_mul(1_u64 << exponent)
        .min(capped_delay_ms);

    let jitter = delay_ms / 5;
    let jittered_delay = if retry_count.is_multiple_of(2) {
        delay_ms.saturating_add(jitter).min(capped_delay_ms)
    } else {
        delay_ms.saturating_sub(jitter)
    };

    Duration::from_millis(jittered_delay)
}

fn print_current_speed(mbits: f64, duration: Duration, payload_size_bytes: usize) {
    stdout::print(&format!(
        "  {:>6.2} mbit/s | {:>5} in {:>4}ms  ",
        mbits,
        format_bytes(payload_size_bytes),
        duration.as_millis(),
    ));
}

fn print_skipped_sample(duration: Duration, status_code: StatusCode, payload_size_bytes: usize) {
    stdout::print(&format!(
        "  {:>6} mbit/s | {:>5} in {:>4}ms -> status: {}  ",
        "N/A",
        format_bytes(payload_size_bytes),
        duration.as_millis(),
        status_code
    ));
}

fn print_retry_notice(delay: Duration, attempt: u32, max_attempts: u32) {
    let delay_display = format_retry_delay(delay);
    let eta_display = format_retry_eta(delay);
    stdout::print(&format!(
        " retrying in {}{} ({}/{})  ",
        delay_display, eta_display, attempt, max_attempts
    ));
}

fn print_transport_failure(duration: Duration, payload_size_bytes: usize, error: &reqwest::Error) {
    stdout::print(&format!(
        "  {:>6} mbit/s | {:>5} in {:>4}ms -> error: {}  ",
        "N/A",
        format_bytes(payload_size_bytes),
        duration.as_millis(),
        error
    ));
}

fn format_retry_delay(delay: Duration) -> String {
    let total_seconds = delay.as_secs();
    if total_seconds == 0 {
        return format!("{}ms", delay.as_millis());
    }
    if total_seconds < 60 {
        return format!("{total_seconds}s");
    }
    if total_seconds < 3600 {
        return format!("{}m {:02}s", total_seconds / 60, total_seconds % 60);
    }
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    format!("{hours}h {minutes:02}m")
}

fn format_retry_eta(delay: Duration) -> String {
    if delay.as_secs() < 60 {
        return String::new();
    }
    let eta = Zoned::now().saturating_add(delay);
    format!(" (until {})", eta.strftime("%H:%M:%S %Z"))
}

pub fn fetch_metadata(client: &Client) -> Result<Metadata, MeasurementError> {
    fetch_metadata_request(client, BASE_URL, None)
}

fn fetch_metadata_request(
    client: &Client,
    base_url: &str,
    timeout: Option<Duration>,
) -> Result<Metadata, MeasurementError> {
    let mut request = client.get(format!("{base_url}/cdn-cgi/trace"));
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }
    let response = request.send()?.error_for_status()?;
    let status_code = response.status().as_u16();
    let body = response.text()?;
    let trace_data = parse_trace_response(&body);
    let ip = trace_data
        .get("ip")
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok());
    let country = trace_data
        .get("loc")
        .filter(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_uppercase()));
    let colo = trace_data
        .get("colo")
        .filter(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase()));
    match (ip, country, colo) {
        (Some(ip), Some(country), Some(colo)) => Ok(Metadata {
            ip: ip.clone(),
            country: country.clone(),
            colo: colo.clone(),
        }),
        _ => Err(MeasurementError {
            status_code: Some(status_code),
            reason: "metadata response has missing or invalid ip, loc, or colo fields".into(),
        }),
    }
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[derive(Clone)]
    struct MockHttpResponse {
        status_code: u16,
        reason: &'static str,
        headers: Vec<(&'static str, &'static str)>,
        body: &'static str,
    }

    fn spawn_mock_http_server(
        responses: Vec<MockHttpResponse>,
    ) -> (String, Arc<AtomicUsize>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind mock HTTP server");
        let addr = listener
            .local_addr()
            .expect("failed to read mock HTTP server addr");
        listener
            .set_nonblocking(true)
            .expect("failed to set nonblocking mode");
        let served = Arc::new(AtomicUsize::new(0));
        let served_counter = Arc::clone(&served);
        let handle = thread::spawn(move || {
            let mut idx = 0usize;
            let mut idle_since = Instant::now();
            while idx < responses.len() {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut buf = [0_u8; 4096];
                        loop {
                            let count = stream.read(&mut buf).unwrap();
                            assert!(count > 0, "incomplete mock request");
                            request.extend_from_slice(&buf[..count]);
                            if let Some(end) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                                let headers = String::from_utf8_lossy(&request[..end]);
                                let length = headers
                                    .lines()
                                    .filter_map(|line| line.split_once(':'))
                                    .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                                    .map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());
                                if request.len() >= end + 4 + length {
                                    break;
                                }
                            }
                        }

                        let response = &responses[idx];
                        let mut response_head = format!(
                            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                            response.status_code,
                            response.reason,
                            response.body.len(),
                        );
                        let mut delay_before_body = Duration::ZERO;
                        for (header, value) in &response.headers {
                            if header.eq_ignore_ascii_case("X-Test-Delay-Ms") {
                                if let Ok(ms) = value.parse::<u64>() {
                                    delay_before_body = Duration::from_millis(ms);
                                }
                                continue;
                            }
                            response_head.push_str(&format!("{header}: {value}\r\n"));
                        }
                        response_head.push_str("\r\n");

                        stream
                            .write_all(response_head.as_bytes())
                            .expect("failed to write mock response head");
                        if !delay_before_body.is_zero() {
                            thread::sleep(delay_before_body);
                        }
                        if !response.body.is_empty() {
                            stream
                                .write_all(response.body.as_bytes())
                                .expect("failed to write mock response body");
                        }
                        idx += 1;
                        served_counter.store(idx, AtomicOrdering::SeqCst);
                        idle_since = Instant::now();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if idle_since.elapsed() > Duration::from_secs(2) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        (format!("http://{}", addr), served, handle)
    }

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
    fn test_run_tests_retries_429_and_records_success() {
        let responses = vec![
            MockHttpResponse {
                status_code: 429,
                reason: "Too Many Requests",
                headers: vec![("Retry-After", "0")],
                body: "",
            },
            MockHttpResponse {
                status_code: 200,
                reason: "OK",
                headers: vec![],
                body: "ok",
            },
        ];
        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");

        let (measurements, payload_stats) = run_tests_with_sleep(
            &client,
            TestType::Download,
            vec![2],
            RetryRunOptions {
                nr_tests: 1,
                output_format: OutputFormat::None,
                disable_dynamic_max_payload_size: true,
            },
            &base_url,
            |_| {},
        );

        assert_eq!(measurements.len(), 1);
        assert_eq!(payload_stats.len(), 1);
        assert_eq!(payload_stats[0].attempts, 2);
        assert_eq!(payload_stats[0].successes, 1);
        assert_eq!(payload_stats[0].skipped, 1);

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 2);
    }

    #[test]
    fn test_run_tests_retry_delay_uses_retry_streak_not_total_attempts() {
        let mut responses = (0..5)
            .map(|_| MockHttpResponse {
                status_code: 200,
                reason: "OK",
                headers: vec![],
                body: "ok",
            })
            .collect::<Vec<_>>();
        responses.push(MockHttpResponse {
            status_code: 429,
            reason: "Too Many Requests",
            headers: vec![],
            body: "",
        });
        responses.push(MockHttpResponse {
            status_code: 200,
            reason: "OK",
            headers: vec![],
            body: "ok",
        });

        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");
        let observed_delays = Arc::new(Mutex::new(Vec::<Duration>::new()));
        let delay_sink = Arc::clone(&observed_delays);

        let (measurements, payload_stats) = run_tests_with_sleep(
            &client,
            TestType::Download,
            vec![2],
            RetryRunOptions {
                nr_tests: 6,
                output_format: OutputFormat::None,
                disable_dynamic_max_payload_size: true,
            },
            &base_url,
            move |delay| {
                delay_sink
                    .lock()
                    .expect("failed to lock delay sink")
                    .push(delay);
            },
        );

        assert_eq!(measurements.len(), 6);
        assert_eq!(payload_stats.len(), 1);
        assert_eq!(payload_stats[0].attempts, 7);
        assert_eq!(payload_stats[0].successes, 6);
        assert_eq!(payload_stats[0].skipped, 1);
        assert_eq!(
            *observed_delays
                .lock()
                .expect("failed to read observed delays"),
            vec![compute_retry_delay(1, None)]
        );

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 7);
    }

    #[test]
    fn test_run_tests_retry_delay_resets_after_success() {
        let responses = vec![
            MockHttpResponse {
                status_code: 429,
                reason: "Too Many Requests",
                headers: vec![],
                body: "",
            },
            MockHttpResponse {
                status_code: 200,
                reason: "OK",
                headers: vec![],
                body: "ok",
            },
            MockHttpResponse {
                status_code: 429,
                reason: "Too Many Requests",
                headers: vec![],
                body: "",
            },
            MockHttpResponse {
                status_code: 200,
                reason: "OK",
                headers: vec![],
                body: "ok",
            },
        ];

        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");
        let observed_delays = Arc::new(Mutex::new(Vec::<Duration>::new()));
        let delay_sink = Arc::clone(&observed_delays);

        let (measurements, payload_stats) = run_tests_with_sleep(
            &client,
            TestType::Download,
            vec![2],
            RetryRunOptions {
                nr_tests: 2,
                output_format: OutputFormat::None,
                disable_dynamic_max_payload_size: true,
            },
            &base_url,
            move |delay| {
                delay_sink
                    .lock()
                    .expect("failed to lock delay sink")
                    .push(delay);
            },
        );

        assert_eq!(measurements.len(), 2);
        assert_eq!(payload_stats.len(), 1);
        assert_eq!(payload_stats[0].attempts, 4);
        assert_eq!(payload_stats[0].successes, 2);
        assert_eq!(payload_stats[0].skipped, 2);
        assert_eq!(
            *observed_delays
                .lock()
                .expect("failed to read observed delays"),
            vec![compute_retry_delay(1, None), compute_retry_delay(1, None)]
        );

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 4);
    }

    #[test]
    fn test_run_tests_stops_after_max_attempts_on_retryable_failures() {
        let responses = (0..8)
            .map(|_| MockHttpResponse {
                status_code: 429,
                reason: "Too Many Requests",
                headers: vec![("Retry-After", "0")],
                body: "",
            })
            .collect::<Vec<_>>();
        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");

        let (measurements, payload_stats) = run_tests_with_sleep(
            &client,
            TestType::Download,
            vec![2],
            RetryRunOptions {
                nr_tests: 2,
                output_format: OutputFormat::None,
                disable_dynamic_max_payload_size: true,
            },
            &base_url,
            |_| {},
        );

        assert!(measurements.is_empty());
        assert_eq!(payload_stats.len(), 1);
        assert_eq!(payload_stats[0].attempts, 8);
        assert_eq!(payload_stats[0].successes, 0);
        assert_eq!(payload_stats[0].skipped, 8);

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 8);
    }

    #[test]
    fn test_run_tests_does_not_retry_non_retryable_4xx() {
        let responses = vec![MockHttpResponse {
            status_code: 404,
            reason: "Not Found",
            headers: vec![],
            body: "",
        }];
        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");

        let (measurements, payload_stats) = run_tests_with_sleep(
            &client,
            TestType::Download,
            vec![2],
            RetryRunOptions {
                nr_tests: 2,
                output_format: OutputFormat::None,
                disable_dynamic_max_payload_size: true,
            },
            &base_url,
            |_| {},
        );

        assert!(measurements.is_empty());
        assert_eq!(payload_stats.len(), 1);
        assert_eq!(payload_stats[0].attempts, 1);
        assert_eq!(payload_stats[0].successes, 0);
        assert_eq!(payload_stats[0].skipped, 1);

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn p1_retry_budget_is_shared_across_download_and_upload() {
        let responses = vec![
            MockHttpResponse {
                status_code: 429,
                reason: "Retry",
                headers: vec![("Retry-After", "1")],
                body: "",
            },
            MockHttpResponse {
                status_code: 200,
                reason: "OK",
                headers: vec![],
                body: "ok",
            },
            MockHttpResponse {
                status_code: 429,
                reason: "Retry",
                headers: vec![("Retry-After", "1")],
                body: "",
            },
        ];
        let (base_url, served, server) = spawn_mock_http_server(responses);
        let client = Client::builder().no_proxy().build().unwrap();
        let mut control = RunControl::new(RunConfig {
            base_url,
            max_retry_wait: Duration::from_secs(1),
            ..RunConfig::default()
        });
        let options = RetryRunOptions {
            nr_tests: 1,
            output_format: OutputFormat::None,
            disable_dynamic_max_payload_size: true,
        };
        let delays = std::cell::RefCell::new(Vec::new());
        let sleep = |delay| delays.borrow_mut().push(delay);
        let (downloads, _) = run_tests_with_control(
            &client,
            TestType::Download,
            vec![2],
            options,
            &mut control,
            sleep,
        );
        let (uploads, _) = run_tests_with_control(
            &client,
            TestType::Upload,
            vec![2],
            options,
            &mut control,
            sleep,
        );
        server.join().unwrap();
        assert_eq!(served.load(AtomicOrdering::SeqCst), 3);
        assert_eq!(downloads.len(), 1);
        assert!(uploads.is_empty());
        assert_eq!(*delays.borrow(), vec![Duration::from_secs(1)]);
        assert_eq!(
            control.stop_reason,
            Some(crate::run::StopReason::RetryBudget)
        );
    }

    #[test]
    fn p1_retry_after_supports_dates_and_handles_invalid_and_huge_delays() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let future = httpdate::fmt_http_date(now + Duration::from_secs(300));
        let past = httpdate::fmt_http_date(now - Duration::from_secs(300));
        for (input, expected) in [
            (future.as_str(), Some(Duration::from_secs(300))),
            (past.as_str(), Some(Duration::ZERO)),
            ("0", Some(Duration::ZERO)),
            (" 42 ", Some(Duration::from_secs(42))),
            (
                "999999999999999999999999999999999999",
                Some(Duration::from_secs(u64::MAX)),
            ),
            ("-1", None),
            ("1.5", None),
            ("nonsense", None),
            ("", None),
        ] {
            let header = reqwest::header::HeaderValue::from_str(input).unwrap();
            assert_eq!(
                parse_retry_after_at(Some(&header), now),
                expected,
                "{input}"
            );
        }
        assert_eq!(parse_retry_after_at(None, now), None);
        assert_eq!(compute_retry_delay(999, None), Duration::from_millis(2400));
    }

    #[test]
    fn test_upload_duration_excludes_delayed_response_body() {
        let responses = vec![MockHttpResponse {
            status_code: 200,
            reason: "OK",
            headers: vec![("X-Test-Delay-Ms", "300")],
            body: "ok",
        }];
        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");

        let wall_start = Instant::now();
        let outcome = test_upload_with_base_url(&client, 100_000, OutputFormat::None, &base_url);
        let wall_elapsed = wall_start.elapsed();

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 1);

        match outcome {
            SampleOutcome::Success { duration, .. } => {
                assert!(
                    duration < Duration::from_millis(200),
                    "upload duration should stop before delayed response drain: {duration:?}"
                );
                assert!(
                    wall_elapsed >= Duration::from_millis(250),
                    "overall call should include delayed body drain: {wall_elapsed:?}"
                );
            }
            other => panic!("expected upload success, got {other:?}"),
        }
    }

    #[test]
    fn test_upload_retryable_failure_parses_retry_after_without_drain_skew() {
        let responses = vec![MockHttpResponse {
            status_code: 429,
            reason: "Too Many Requests",
            headers: vec![("Retry-After", "1"), ("X-Test-Delay-Ms", "300")],
            body: "retry later",
        }];
        let (base_url, served_counter, handle) = spawn_mock_http_server(responses);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build test client");

        let wall_start = Instant::now();
        let outcome = test_upload_with_base_url(&client, 100_000, OutputFormat::None, &base_url);
        let wall_elapsed = wall_start.elapsed();

        handle.join().expect("mock server thread panicked");
        assert_eq!(served_counter.load(AtomicOrdering::SeqCst), 1);

        match outcome {
            SampleOutcome::RetryableFailure {
                duration,
                status_code,
                retry_after,
                ..
            } => {
                assert!(
                    duration < Duration::from_millis(200),
                    "retryable failure duration should stop before delayed body drain: {duration:?}"
                );
                assert_eq!(status_code, Some(StatusCode::TOO_MANY_REQUESTS));
                assert_eq!(retry_after, Some(Duration::from_secs(1)));
                assert!(
                    wall_elapsed >= Duration::from_millis(250),
                    "overall call should include delayed body drain: {wall_elapsed:?}"
                );
            }
            other => panic!("expected retryable upload failure, got {other:?}"),
        }
    }

    #[test]
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

    #[test]
    fn test_parse_latency_from_legacy_header() {
        // Old cfRequestDuration format
        let header = "cfRequestDuration;dur=3.456";
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(
            result.is_some(),
            "Should parse legacy cfRequestDuration header"
        );
        let latency = result.unwrap();
        // latency = total_ms - server_duration = 50.0 - 3.456 = 46.544
        assert!((latency - 46.544).abs() < 0.001);
    }

    #[test]
    fn test_parse_latency_from_cfl4_header() {
        // New cfL4 format - rtt is in microseconds
        let header =
            r#"cfL4;desc="?proto=TCP&rtt=5003&min_rtt=4257&rtt_var=2477&sent=6&recv=6&lost=0""#;
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(result.is_some(), "Should parse cfL4 rtt header");
        let latency = result.unwrap();
        // rtt=5003 microseconds = 5.003 milliseconds
        assert!((latency - 5.003).abs() < 0.001);
    }

    #[test]
    fn test_parse_latency_missing_header() {
        let header = "some-unrelated;value=123";
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(
            result.is_none(),
            "Should return None for unrecognized header"
        );
    }

    #[test]
    fn test_parse_latency_prefers_legacy_over_cfl4() {
        // If both are present (unlikely but defensive), prefer legacy
        let header = "cfRequestDuration;dur=3.456, cfL4;desc=\"?proto=TCP&rtt=5003\"";
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(result.is_some());
        let latency = result.unwrap();
        assert!((latency - 46.544).abs() < 0.001);
    }

    #[test]
    fn test_parse_latency_cfl4_zero_rtt() {
        let header = r#"cfL4;desc="?proto=TCP&rtt=0&min_rtt=0""#;
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(result.is_some());
        assert!((result.unwrap() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_latency_negative_clamp() {
        // Legacy header where server processing > total RTT (clock skew)
        let header = "cfRequestDuration;dur=100.0";
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(result.is_some());
        // Should clamp to 0, not return negative
        assert!((result.unwrap() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_latency_cfl4_does_not_match_min_rtt() {
        // Regression: rtt= regex must not match min_rtt= or rtt_var=
        // Header with min_rtt before rtt - if regex is naive, it grabs min_rtt's value
        let header = r#"cfL4;desc="?proto=TCP&min_rtt=4257&rtt_var=2477&rtt=5003&sent=6""#;
        let total_ms = 50.0;
        let result = parse_latency_from_server_timing(header, total_ms);
        assert!(result.is_some());
        let latency = result.unwrap();
        // Must match rtt=5003, NOT min_rtt=4257
        assert!((latency - 5.003).abs() < 0.001);
    }

    #[test]
    fn test_test_latency_no_panic_on_request_failure() {
        // Proxy pointed at a port with nothing listening - instant connection refused.
        let client = reqwest::blocking::Client::builder()
            .proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap())
            .build()
            .unwrap();
        // Must not panic or return a plausible latency on failure.
        let result = test_latency(&client);
        assert!(result.is_nan());
    }

    #[test]
    fn test_run_latency_test_all_failures_returns_nan_avg() {
        // Every request fails immediately - failed samples are skipped
        let client = reqwest::blocking::Client::builder()
            .proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap())
            .build()
            .unwrap();
        let (measurements, avg) = run_latency_test(&client, 3, OutputFormat::Json);
        assert!(measurements.is_empty(), "Failed requests should be skipped");
        assert!(
            avg.is_nan(),
            "Missing average must not be a zero measurement"
        );
    }
}

#[cfg(test)]
mod p1_regressions {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn download_response(response: &'static [u8], pause: Duration) -> SampleOutcome {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 4096];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let received = stream.read(&mut buffer).unwrap();
                assert!(received > 0, "incomplete mock request headers");
                request.extend_from_slice(&buffer[..received]);
            }
            stream.write_all(response).unwrap();
            thread::sleep(pause);
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let outcome = test_download_with_base_url(&client, 4, OutputFormat::None, &url);
        server.join().unwrap();
        outcome
    }

    #[test]
    fn p1_download_rejects_truncated_body() {
        let result = download_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nok",
            Duration::ZERO,
        );
        assert!(
            !matches!(result, SampleOutcome::Success { .. }),
            "{result:?}"
        );
    }

    #[test]
    fn p1_download_rejects_incorrect_size() {
        for response in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice(),
            b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".as_slice(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n12345".as_slice(),
        ] {
            let result = download_response(response, Duration::ZERO);
            assert!(
                !matches!(result, SampleOutcome::Success { .. }),
                "{result:?}"
            );
        }
    }

    #[test]
    fn p1_download_rejects_body_timeout() {
        let result = download_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n",
            Duration::from_millis(250),
        );
        assert!(
            !matches!(result, SampleOutcome::Success { .. }),
            "{result:?}"
        );
    }

    #[test]
    fn p1_download_accepts_complete_chunked_body() {
        let result = download_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n", Duration::ZERO);
        assert!(
            matches!(result, SampleOutcome::Success { mbits, .. } if mbits.is_finite() && mbits > 0.0),
            "{result:?}"
        );
    }
}
