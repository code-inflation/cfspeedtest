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

pub(crate) fn log_measurements(measurements: &[Measurement]) {
    println!("\n### STATS ###");
    measurements
        .iter()
        .map(|m| m.test_type)
        .collect::<HashSet<TestType>>()
        .iter()
        .for_each(|t| log_measurements_by_test_type(measurements, *t));
}

fn log_measurements_by_test_type(measurements: &[Measurement], test_type: TestType) {
    // TODO calculate this for each payload size
    let type_measurements: Vec<f64> = measurements
        .iter()
        .filter(|m| m.test_type == test_type)
        .map(|m| m.mbit)
        .collect();
    let (min, max, avg) = calc_stats(type_measurements);
    // TODO draw boxplot etc
    println!("{test_type:?}: min {min:.2}, max {max:.2}, avg {avg:.2}");
}

fn calc_stats(mbit_measurements: Vec<f64>) -> (f64, f64, f64) {
    let min = mbit_measurements
        .iter()
        .fold(f64::INFINITY, |a, b| a.min(*b));
    let max = mbit_measurements
        .iter()
        .fold(f64::NEG_INFINITY, |a, b| a.max(*b));
    let avg: f64 = mbit_measurements.iter().sum::<f64>() / mbit_measurements.len() as f64;
    (min, max, avg)
}

pub(crate) fn format_bytes(bytes: usize) -> String {
    match bytes {
        1_000..=999_999 => format!("{}KB", bytes / 1_000),
        1_000_000..=999_999_999 => format!("{}MB", bytes / 1_000_000),
        _ => format!("{bytes} bytes"),
    }
}
