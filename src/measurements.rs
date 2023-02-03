use crate::speedtest::TestType;
use std::fmt::Display;

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
    // TODO calculate this for each payload size
    let min = measurements
        .iter()
        .map(|m| m.mbit)
        .fold(f64::INFINITY, |a, b| a.min(b));
    let max = measurements
        .iter()
        .map(|m| m.mbit)
        .fold(f64::NEG_INFINITY, |a, b| a.max(b));
    let avg: f64 = measurements.iter().map(|m| m.mbit).sum::<f64>() / measurements.len() as f64;

    // TODO draw boxplot etc
    println!(
        "{:?}: min {:.2}, max {:.2}, avg {:.2}\n",
        measurements[0].test_type, min, max, avg
    );
}

pub(crate) fn format_bytes(bytes: usize) -> String {
    match bytes {
        1_000..=999_999 => format!("{}KB", bytes / 1_000),
        1_000_000..=999_999_999 => format!("{}MB", bytes / 1_000_000),
        _ => format!("{bytes} bytes"),
    }
}
