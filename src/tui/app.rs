use crate::engine::types::*;

/// Current phase of the speed test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Latency,
    Download,
    Upload,
    Results,
}

/// TUI application state, updated by consuming SpeedTestEvents.
pub struct App {
    pub phase: Phase,
    pub metadata: Option<Metadata>,

    // Latency
    pub latency_samples: Vec<f64>,
    pub latency_index: u32,
    pub latency_total: u32,
    pub latency_result: Option<LatencyResult>,

    // Throughput
    pub current_test_type: Option<TestType>,
    pub current_payload_size: Option<PayloadSize>,
    pub current_mbps: f64,
    pub throughput_samples: Vec<(TestType, PayloadSize, f64)>,
    pub throughput_index: u32,
    pub throughput_total: u32,
    pub chart_data: Vec<f64>,

    // Transfer progress
    pub transfer_bytes: u64,
    pub transfer_total: u64,
    pub transfer_mbps: f64,

    // Results
    pub download_result: Option<ThroughputResult>,
    pub upload_result: Option<ThroughputResult>,
    pub final_result: Option<SpeedTestResult>,

    // Skipped payloads
    pub skipped: Vec<(TestType, PayloadSize)>,

    // Errors
    pub errors: Vec<String>,

    pub should_quit: bool,
}

impl App {
    pub fn new(latency_total: u32) -> Self {
        Self {
            phase: Phase::Connecting,
            metadata: None,
            latency_samples: Vec::new(),
            latency_index: 0,
            latency_total,
            latency_result: None,
            current_test_type: None,
            current_payload_size: None,
            current_mbps: 0.0,
            throughput_samples: Vec::new(),
            throughput_index: 0,
            throughput_total: 0,
            chart_data: Vec::new(),
            transfer_bytes: 0,
            transfer_total: 0,
            transfer_mbps: 0.0,
            download_result: None,
            upload_result: None,
            final_result: None,
            skipped: Vec::new(),
            errors: Vec::new(),
            should_quit: false,
        }
    }

    /// Process a speed test event and update state.
    pub fn handle_event(&mut self, event: SpeedTestEvent) {
        match event {
            SpeedTestEvent::MetadataReady(meta) => {
                self.metadata = Some(meta);
                self.phase = Phase::Latency;
            }
            SpeedTestEvent::LatencySample {
                rtt_ms,
                index,
                total,
            } => {
                self.latency_samples.push(rtt_ms);
                self.latency_index = index;
                self.latency_total = total;
            }
            SpeedTestEvent::LatencyComplete(result) => {
                self.latency_result = Some(result);
            }
            SpeedTestEvent::PhaseStart(test_type) => {
                self.phase = match test_type {
                    TestType::Download => Phase::Download,
                    TestType::Upload => Phase::Upload,
                };
                self.current_test_type = Some(test_type);
                self.chart_data.clear();
                self.throughput_index = 0;
                self.current_mbps = 0.0;
            }
            SpeedTestEvent::ThroughputSample {
                test_type,
                payload_size,
                mbps,
                index,
                total,
            } => {
                self.throughput_samples
                    .push((test_type, payload_size, mbps));
                self.current_payload_size = Some(payload_size);
                self.current_mbps = mbps;
                self.throughput_index = index;
                self.throughput_total = total;
                self.chart_data.push(mbps);
            }
            SpeedTestEvent::TransferProgress {
                bytes_so_far,
                total_bytes,
                current_mbps,
                ..
            } => {
                self.transfer_bytes = bytes_so_far;
                self.transfer_total = total_bytes;
                self.transfer_mbps = current_mbps;
            }
            SpeedTestEvent::PayloadSkipped {
                test_type,
                payload_size,
            } => {
                self.skipped.push((test_type, payload_size));
            }
            SpeedTestEvent::ThroughputComplete { test_type, result } => match test_type {
                TestType::Download => self.download_result = Some(result),
                TestType::Upload => self.upload_result = Some(result),
            },
            SpeedTestEvent::Complete(result) => {
                self.final_result = Some(result);
                self.phase = Phase::Results;
            }
            SpeedTestEvent::Error(msg) => {
                self.errors.push(msg);
            }
        }
    }

    /// Current overall progress as fraction (0.0..1.0).
    pub fn overall_progress(&self) -> f64 {
        match self.phase {
            Phase::Connecting => 0.0,
            Phase::Latency => {
                if self.latency_total == 0 {
                    0.0
                } else {
                    (self.latency_index as f64 / self.latency_total as f64) * 0.2
                }
            }
            Phase::Download => {
                0.2 + if self.throughput_total == 0 {
                    0.0
                } else {
                    (self.throughput_index as f64 / self.throughput_total as f64) * 0.4
                }
            }
            Phase::Upload => {
                0.6 + if self.throughput_total == 0 {
                    0.0
                } else {
                    (self.throughput_index as f64 / self.throughput_total as f64) * 0.4
                }
            }
            Phase::Results => 1.0,
        }
    }
}
