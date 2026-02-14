use crate::boxplot;
use crate::speedtest::Metadata;
use crate::speedtest::TestType;
use crate::OutputFormat;
use indexmap::IndexSet;
use serde::Serialize;
use std::{fmt::Display, io};

#[derive(Serialize)]
struct StatMeasurement {
    test_type: TestType,
    payload_size: usize,
    min: Option<f64>,
    q1: Option<f64>,
    median: Option<f64>,
    q3: Option<f64>,
    max: Option<f64>,
    avg: Option<f64>,
    attempts: u32,
    successes: u32,
    skipped: u32,
    target_successes: u32,
}

#[derive(Serialize)]
pub struct Measurement {
    pub test_type: TestType,
    pub payload_size: usize,
    pub mbit: f64,
}

#[derive(Serialize)]
pub struct LatencyMeasurement {
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub latency_measurements: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PayloadAttemptStats {
    pub test_type: TestType,
    pub payload_size: usize,
    pub attempts: u32,
    pub successes: u32,
    pub skipped: u32,
    pub target_successes: u32,
}

impl Display for Measurement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: \t{}\t-> {}",
            self.test_type,
            format_bytes(self.payload_size),
            self.mbit,
        )
    }
}

pub(crate) fn log_measurements(
    measurements: &[Measurement],
    payload_attempt_stats: &[PayloadAttemptStats],
    latency_measurement: Option<&LatencyMeasurement>,
    payload_sizes: Vec<usize>,
    verbose: bool,
    output_format: OutputFormat,
    metadata: Option<&Metadata>,
) {
    if output_format == OutputFormat::StdOut {
        println!("\nSummary Statistics");
        if verbose {
            println!("Type     Payload |  min/max/avg in mbit/s | attempts/success/skipped");
        } else {
            println!("Type     Payload |  min/max/avg in mbit/s");
        }
    }
    let mut stat_measurements: Vec<StatMeasurement> = Vec::new();
    let mut test_types = measurements
        .iter()
        .map(|m| m.test_type)
        .collect::<IndexSet<TestType>>();
    payload_attempt_stats
        .iter()
        .for_each(|stats| _ = test_types.insert(stats.test_type));

    test_types.iter().for_each(|test_type| {
        stat_measurements.extend(log_measurements_by_test_type(
            measurements,
            payload_attempt_stats,
            payload_sizes.clone(),
            verbose,
            output_format,
            *test_type,
        ))
    });
    match output_format {
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(io::stdout());
            for measurement in &stat_measurements {
                wtr.serialize(measurement).unwrap();
            }
            wtr.flush().unwrap();
        }
        OutputFormat::Json => {
            let output = compose_output_json(&stat_measurements, latency_measurement, metadata);
            serde_json::to_writer(io::stdout(), &output).unwrap();
            println!();
        }
        OutputFormat::JsonPretty => {
            let output = compose_output_json(&stat_measurements, latency_measurement, metadata);
            serde_json::to_writer_pretty(io::stdout(), &output).unwrap();
            println!();
        }
        OutputFormat::StdOut => {}
        OutputFormat::None => {}
    }
}

fn compose_output_json(
    stat_measurements: &[StatMeasurement],
    latency_measurement: Option<&LatencyMeasurement>,
    metadata: Option<&Metadata>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut output = serde_json::Map::new();
    if let Some(metadata) = metadata {
        output.insert(
            "metadata".to_string(),
            serde_json::to_value(metadata).unwrap(),
        );
    }
    if let Some(latency) = latency_measurement {
        output.insert(
            "latency_measurement".to_string(),
            serde_json::to_value(latency).unwrap(),
        );
    }
    output.insert(
        "speed_measurements".to_string(),
        serde_json::to_value(stat_measurements).unwrap(),
    );
    output
}

fn log_measurements_by_test_type(
    measurements: &[Measurement],
    payload_attempt_stats: &[PayloadAttemptStats],
    payload_sizes: Vec<usize>,
    verbose: bool,
    output_format: OutputFormat,
    test_type: TestType,
) -> Vec<StatMeasurement> {
    let mut stat_measurements: Vec<StatMeasurement> = Vec::new();
    for payload_size in payload_sizes {
        let type_measurements: Vec<f64> = measurements
            .iter()
            .filter(|m| m.test_type == test_type)
            .filter(|m| m.payload_size == payload_size)
            .map(|m| m.mbit)
            .collect();
        let payload_attempt_stat = payload_attempt_stats
            .iter()
            .find(|stats| stats.test_type == test_type && stats.payload_size == payload_size);

        if type_measurements.is_empty() && payload_attempt_stat.is_none() {
            continue;
        }

        let attempts = payload_attempt_stat.map_or(0, |stats| stats.attempts);
        let successes = payload_attempt_stat.map_or(0, |stats| stats.successes);
        let skipped = payload_attempt_stat.map_or(0, |stats| stats.skipped);
        let target_successes = payload_attempt_stat.map_or(0, |stats| stats.target_successes);

        let formatted_payload = format_bytes(payload_size);
        let fmt_test_type = format!("{test_type:?}");

        if !type_measurements.is_empty() {
            let (min, q1, median, q3, max, avg) = calc_stats(type_measurements).unwrap();

            stat_measurements.push(StatMeasurement {
                test_type,
                payload_size,
                min: Some(min),
                q1: Some(q1),
                median: Some(median),
                q3: Some(q3),
                max: Some(max),
                avg: Some(avg),
                attempts,
                successes,
                skipped,
                target_successes,
            });
            if output_format == OutputFormat::StdOut {
                if verbose {
                    println!(
                        "{fmt_test_type:<9} {formatted_payload:<7}|  min {min:<7.2} max {max:<7.2} avg {avg:<7.2} | {attempts:>3}/{successes:>3}/{skipped:>3}"
                    );
                } else {
                    println!(
                        "{fmt_test_type:<9} {formatted_payload:<7}|  min {min:<7.2} max {max:<7.2} avg {avg:<7.2}"
                    );
                }
                if successes < target_successes {
                    println!(
                        "                    insufficient samples: collected {successes}/{target_successes} successful runs"
                    );
                }
                if verbose {
                    let plot = boxplot::render_plot(min, q1, median, q3, max);
                    println!("{plot}\n");
                }
            }
        } else {
            stat_measurements.push(StatMeasurement {
                test_type,
                payload_size,
                min: None,
                q1: None,
                median: None,
                q3: None,
                max: None,
                avg: None,
                attempts,
                successes,
                skipped,
                target_successes,
            });
            if output_format == OutputFormat::StdOut {
                if verbose {
                    println!(
                        "{fmt_test_type:<9} {formatted_payload:<7}|  min N/A     max N/A     avg N/A     | {attempts:>3}/{successes:>3}/{skipped:>3} (insufficient samples)"
                    );
                } else {
                    println!(
                        "{fmt_test_type:<9} {formatted_payload:<7}|  min N/A     max N/A     avg N/A     (insufficient samples)"
                    );
                }
            }
        }
    }

    stat_measurements
}

fn calc_stats(mbit_measurements: Vec<f64>) -> Option<(f64, f64, f64, f64, f64, f64)> {
    log::debug!("calc_stats for mbit_measurements {mbit_measurements:?}");
    let length = mbit_measurements.len();
    if length == 0 {
        return None;
    }

    let mut sorted_data = mbit_measurements.clone();
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less));

    if length == 1 {
        return Some((
            sorted_data[0],
            sorted_data[0],
            sorted_data[0],
            sorted_data[0],
            sorted_data[0],
            sorted_data[0],
        ));
    }

    if length < 4 {
        return Some((
            *sorted_data.first().unwrap(),
            *sorted_data.first().unwrap(),
            median(&sorted_data),
            *sorted_data.last().unwrap(),
            *sorted_data.last().unwrap(),
            mbit_measurements.iter().sum::<f64>() / mbit_measurements.len() as f64,
        ));
    }

    let q1 = if length.is_multiple_of(2) {
        median(&sorted_data[0..length / 2])
    } else {
        median(&sorted_data[0..length.div_ceil(2)])
    };

    let q3 = if length.is_multiple_of(2) {
        median(&sorted_data[length / 2..length])
    } else {
        median(&sorted_data[length.div_ceil(2)..length])
    };

    Some((
        *sorted_data.first().unwrap(),
        q1,
        median(&sorted_data),
        q3,
        *sorted_data.last().unwrap(),
        mbit_measurements.iter().sum::<f64>() / mbit_measurements.len() as f64,
    ))
}

fn median(data: &[f64]) -> f64 {
    let length = data.len();
    if length.is_multiple_of(2) {
        (data[length / 2 - 1] + data[length / 2]) / 2.0
    } else {
        data[length / 2]
    }
}

pub(crate) fn format_bytes(bytes: usize) -> String {
    match bytes {
        1_000..=999_999 => format!("{}KB", bytes / 1_000),
        1_000_000..=999_999_999 => format!("{}MB", bytes / 1_000_000),
        _ => format!("{bytes} bytes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 bytes");
        assert_eq!(format_bytes(1_000), "1KB");
        assert_eq!(format_bytes(100_000), "100KB");
        assert_eq!(format_bytes(999_999), "999KB");
        assert_eq!(format_bytes(1_000_000), "1MB");
        assert_eq!(format_bytes(25_000_000), "25MB");
        assert_eq!(format_bytes(100_000_000), "100MB");
        assert_eq!(format_bytes(999_999_999), "999MB");
        assert_eq!(format_bytes(1_000_000_000), "1000000000 bytes");
    }

    #[test]
    fn test_measurement_display() {
        let measurement = Measurement {
            test_type: TestType::Download,
            payload_size: 1_000_000,
            mbit: 50.5,
        };

        let display_str = format!("{measurement}");
        assert!(display_str.contains("Download"));
        assert!(display_str.contains("1MB"));
        assert!(display_str.contains("50.5"));
    }

    #[test]
    fn test_calc_stats_empty() {
        assert_eq!(calc_stats(vec![]), None);
    }

    #[test]
    fn test_calc_stats_single_value() {
        let result = calc_stats(vec![10.0]).unwrap();
        assert_eq!(result, (10.0, 10.0, 10.0, 10.0, 10.0, 10.0));
    }

    #[test]
    fn test_calc_stats_two_values() {
        let result = calc_stats(vec![10.0, 20.0]).unwrap();
        assert_eq!(result.0, 10.0); // min
        assert_eq!(result.4, 20.0); // max
        assert_eq!(result.2, 15.0); // median
        assert_eq!(result.5, 15.0); // avg
    }

    #[test]
    fn test_calc_stats_multiple_values() {
        let result = calc_stats(vec![1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();
        assert_eq!(result.0, 1.0); // min
        assert_eq!(result.4, 5.0); // max
        assert_eq!(result.2, 3.0); // median
        assert_eq!(result.5, 3.0); // avg
    }

    #[test]
    fn test_calc_stats_unsorted() {
        let result = calc_stats(vec![5.0, 1.0, 3.0, 2.0, 4.0]).unwrap();
        assert_eq!(result.0, 1.0); // min
        assert_eq!(result.4, 5.0); // max
        assert_eq!(result.2, 3.0); // median
        assert_eq!(result.5, 3.0); // avg
    }

    #[test]
    fn test_median_odd_length() {
        assert_eq!(median(&[1.0, 2.0, 3.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
    }

    #[test]
    fn test_median_even_length() {
        assert_eq!(median(&[1.0, 2.0]), 1.5);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
    }

    #[test]
    fn test_median_single_value() {
        assert_eq!(median(&[5.0]), 5.0);
    }

    #[test]
    fn test_compose_output_json_includes_metadata() {
        let stat_measurements = vec![StatMeasurement {
            test_type: TestType::Download,
            payload_size: 100_000,
            min: Some(1.0),
            q1: Some(1.5),
            median: Some(2.0),
            q3: Some(2.5),
            max: Some(3.0),
            avg: Some(2.0),
            attempts: 3,
            successes: 3,
            skipped: 0,
            target_successes: 3,
        }];
        let latency = LatencyMeasurement {
            avg_latency_ms: 10.0,
            min_latency_ms: 9.0,
            max_latency_ms: 11.0,
            latency_measurements: vec![9.0, 10.0, 11.0],
        };
        let metadata = Metadata {
            country: "Country".to_string(),
            ip: "127.0.0.1".to_string(),
            colo: "ABC".to_string(),
        };

        let output =
            super::compose_output_json(&stat_measurements, Some(&latency), Some(&metadata));

        let metadata_value = output.get("metadata").expect("metadata missing");
        let metadata_obj = metadata_value.as_object().expect("metadata not an object");
        assert_eq!(
            metadata_obj.get("country").and_then(|v| v.as_str()),
            Some("Country")
        );
        assert_eq!(
            metadata_obj.get("ip").and_then(|v| v.as_str()),
            Some("127.0.0.1")
        );
        assert_eq!(
            metadata_obj.get("colo").and_then(|v| v.as_str()),
            Some("ABC")
        );

        assert!(output.get("latency_measurement").is_some());
        assert!(output.get("speed_measurements").is_some());

        let keys: Vec<&str> = output.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec!["metadata", "latency_measurement", "speed_measurements"]
        );
    }
}
