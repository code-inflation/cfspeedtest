use crate::boxplot;
use crate::speedtest::TestType;
use crate::OutputFormat;
use indexmap::IndexSet;
use serde::Serialize;
use std::{fmt::Display, io};

#[derive(Serialize)]
struct StatMeasurement {
    test_type: TestType,
    payload_size: usize,
    min: f64,
    q1: f64,
    median: f64,
    q3: f64,
    max: f64,
    avg: f64,
}

#[derive(Serialize)]
pub struct Measurement {
    pub test_type: TestType,
    pub payload_size: usize,
    pub mbit: f64,
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
    payload_sizes: Vec<usize>,
    verbose: bool,
    output_format: OutputFormat,
) {
    if output_format == OutputFormat::StdOut {
        println!("\nSummary Statistics");
        println!("Type     Payload |  min/max/avg in mbit/s");
    }
    let mut stat_measurements: Vec<StatMeasurement> = Vec::new();
    measurements
        .iter()
        .map(|m| m.test_type)
        .collect::<IndexSet<TestType>>()
        .iter()
        .for_each(|t| {
            stat_measurements.extend(log_measurements_by_test_type(
                measurements,
                payload_sizes.clone(),
                verbose,
                output_format,
                *t,
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
            serde_json::to_writer(io::stdout(), &stat_measurements).unwrap();
            println!();
        }
        OutputFormat::JsonPretty => {
            // json_pretty output test
            serde_json::to_writer_pretty(io::stdout(), &stat_measurements).unwrap();
            println!();
        }
        OutputFormat::StdOut => {}
        OutputFormat::None => {}
    }
}

fn log_measurements_by_test_type(
    measurements: &[Measurement],
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

        // check if there are any measurements for the current payload_size
        // skip stats calculation if there are no measurements
        if !type_measurements.is_empty() {
            let (min, q1, median, q3, max, avg) = calc_stats(type_measurements).unwrap();

            let formatted_payload = format_bytes(payload_size);
            let fmt_test_type = format!("{test_type:?}");
            stat_measurements.push(StatMeasurement {
                test_type,
                payload_size,
                min,
                q1,
                median,
                q3,
                max,
                avg,
            });
            if output_format == OutputFormat::StdOut {
                println!(
                "{fmt_test_type:<9} {formatted_payload:<7}|  min {min:<7.2} max {max:<7.2} avg {avg:<7.2}"
            );
                if verbose {
                    let plot = boxplot::render_plot(min, q1, median, q3, max);
                    println!("{plot}\n");
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
}
