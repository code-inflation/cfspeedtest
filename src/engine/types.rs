use serde::Serialize;
use std::fmt;
use tokio::sync::mpsc;

/// Cloudflare server metadata from the trace endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct Metadata {
    pub ip: String,
    pub colo: String,
    pub country: String,
}

/// Which direction a throughput test measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TestType {
    Download,
    Upload,
}

impl fmt::Display for TestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestType::Download => write!(f, "Download"),
            TestType::Upload => write!(f, "Upload"),
        }
    }
}

/// Payload sizes used for throughput tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum PayloadSize {
    K100,
    M1,
    M10,
    M25,
    M100,
}

impl PayloadSize {
    pub fn bytes(self) -> usize {
        match self {
            PayloadSize::K100 => 100_000,
            PayloadSize::M1 => 1_000_000,
            PayloadSize::M10 => 10_000_000,
            PayloadSize::M25 => 25_000_000,
            PayloadSize::M100 => 100_000_000,
        }
    }

    /// All sizes up to and including `max`.
    pub fn sizes_up_to(max: PayloadSize) -> Vec<PayloadSize> {
        use PayloadSize::*;
        let all = [K100, M1, M10, M25, M100];
        all.into_iter().filter(|s| *s <= max).collect()
    }
}

impl fmt::Display for PayloadSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PayloadSize::K100 => write!(f, "100KB"),
            PayloadSize::M1 => write!(f, "1MB"),
            PayloadSize::M10 => write!(f, "10MB"),
            PayloadSize::M25 => write!(f, "25MB"),
            PayloadSize::M100 => write!(f, "100MB"),
        }
    }
}

/// A single throughput measurement.
#[derive(Debug, Clone, Serialize)]
pub struct Measurement {
    pub test_type: TestType,
    pub payload_size: PayloadSize,
    pub mbps: f64,
}

/// Aggregated latency results.
#[derive(Debug, Clone, Serialize)]
pub struct LatencyResult {
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub samples: Vec<f64>,
}

/// Per-payload-size statistics.
#[derive(Debug, Clone, Serialize)]
pub struct PayloadStats {
    pub test_type: TestType,
    pub payload_size: PayloadSize,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub avg: f64,
}

/// Final result of a complete speed test run.
#[derive(Debug, Clone, Serialize)]
pub struct SpeedTestResult {
    pub metadata: Metadata,
    pub latency: Option<LatencyResult>,
    pub download: Option<ThroughputResult>,
    pub upload: Option<ThroughputResult>,
}

/// Aggregated throughput result for one direction.
#[derive(Debug, Clone, Serialize)]
pub struct ThroughputResult {
    pub overall_mbps: f64,
    pub measurements: Vec<Measurement>,
    pub stats: Vec<PayloadStats>,
}

/// Events emitted by the engine for real-time consumption.
#[derive(Debug, Clone)]
pub enum SpeedTestEvent {
    MetadataReady(Metadata),
    LatencySample {
        rtt_ms: f64,
        index: u32,
        total: u32,
    },
    LatencyComplete(LatencyResult),
    PhaseStart(TestType),
    ThroughputSample {
        test_type: TestType,
        payload_size: PayloadSize,
        mbps: f64,
        index: u32,
        total: u32,
    },
    TransferProgress {
        test_type: TestType,
        bytes_so_far: u64,
        total_bytes: u64,
        current_mbps: f64,
    },
    PayloadSkipped {
        test_type: TestType,
        payload_size: PayloadSize,
    },
    ThroughputComplete {
        test_type: TestType,
        result: ThroughputResult,
    },
    Complete(SpeedTestResult),
    Error(String),
}

/// Configuration for a speed test run.
#[derive(Debug, Clone)]
pub struct SpeedTestConfig {
    pub nr_tests: u32,
    pub nr_latency_tests: u32,
    pub max_payload_size: PayloadSize,
    pub disable_dynamic_max_payload_size: bool,
    pub download: bool,
    pub upload: bool,
}

impl Default for SpeedTestConfig {
    fn default() -> Self {
        Self {
            nr_tests: 10,
            nr_latency_tests: 25,
            max_payload_size: PayloadSize::M25,
            disable_dynamic_max_payload_size: false,
            download: true,
            upload: true,
        }
    }
}

pub type EventSender = mpsc::Sender<SpeedTestEvent>;

/// Compute statistics for a slice of f64 values.
pub fn calc_stats(values: &[f64]) -> (f64, f64, f64, f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let avg = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let median = calc_median(&sorted);

    let (lower, upper) = if sorted.len().is_multiple_of(2) {
        let mid = sorted.len() / 2;
        (&sorted[..mid], &sorted[mid..])
    } else {
        let mid = sorted.len().div_ceil(2);
        (&sorted[..mid], &sorted[sorted.len() - mid..])
    };
    let q1 = calc_median(lower);
    let q3 = calc_median(upper);

    (min, q1, median, q3, max, avg)
}

fn calc_median(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Format bytes into a human-readable string.
pub fn format_bytes(bytes: usize) -> String {
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
    fn test_payload_sizes_up_to() {
        let sizes = PayloadSize::sizes_up_to(PayloadSize::M10);
        assert_eq!(
            sizes,
            vec![PayloadSize::K100, PayloadSize::M1, PayloadSize::M10]
        );
    }

    #[test]
    fn test_calc_stats_basic() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (min, _q1, median, _q3, max, avg) = calc_stats(&vals);
        assert_eq!(min, 1.0);
        assert_eq!(max, 5.0);
        assert_eq!(median, 3.0);
        assert_eq!(avg, 3.0);
    }

    #[test]
    fn test_calc_stats_empty() {
        let (min, q1, median, q3, max, avg) = calc_stats(&[]);
        assert_eq!(
            (min, q1, median, q3, max, avg),
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(100_000), "100KB");
        assert_eq!(format_bytes(1_000_000), "1MB");
        assert_eq!(format_bytes(25_000_000), "25MB");
    }
}
