use crate::boxplot;
use crate::speedtest::TestType;
use std::{collections::HashSet, fmt::Display};

pub(crate) struct Measurement {
    pub(crate) test_type: TestType,
    pub(crate) payload_size: usize,
    pub(crate) mbit: f64,
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
) {
    println!("\n### STATS ###");
    measurements
        .iter()
        .map(|m| m.test_type)
        .collect::<HashSet<TestType>>()
        .iter()
        .for_each(|t| {
            log_measurements_by_test_type(measurements, payload_sizes.clone(), verbose, *t)
        });
}

fn log_measurements_by_test_type(
    measurements: &[Measurement],
    payload_sizes: Vec<usize>,
    verbose: bool,
    test_type: TestType,
) {
    for payload_size in payload_sizes {
        let type_measurements: Vec<f64> = measurements
            .iter()
            .filter(|m| m.test_type == test_type)
            .filter(|m| m.payload_size == payload_size)
            .map(|m| m.mbit)
            .collect();
        let (min, q1, median, q3, max, avg) = calc_stats(type_measurements).unwrap();

        let formated_payload = format_bytes(payload_size);
        println!("{test_type:?} {formated_payload}: min {min:.2}, max {max:.2}, avg {avg:.2}");
        if verbose {
            let plot = boxplot::render_plot(min, q1, median, q3, max);
            println!("{plot}\n");
        }
    }
}

fn calc_stats(mbit_measurements: Vec<f64>) -> Option<(f64, f64, f64, f64, f64, f64)> {
    log::debug!("calc_stats for mbit_measurements {mbit_measurements:?}");
    let length = mbit_measurements.len();
    if length < 4 {
        return None;
    }

    let mut sorted_data = mbit_measurements.clone();
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less));

    let q1 = if length % 2 == 0 {
        median(&sorted_data[0..length / 2])
    } else {
        median(&sorted_data[0..(length + 1) / 2])
    };

    let q3 = if length % 2 == 0 {
        median(&sorted_data[length / 2..length])
    } else {
        median(&sorted_data[(length + 1) / 2..length])
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
    if length % 2 == 0 {
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
