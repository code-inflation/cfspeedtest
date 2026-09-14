//! Run outcomes and shared limits for metadata, latency, transfers, and retries.
use crate::measurements::{Measurement, PayloadAttemptStats};
use crate::speedtest::Metadata;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RunConfig {
    pub base_url: String,
    pub max_duration: Duration,
    pub max_retry_wait: Duration,
    /// Set this flag to request cancellation. Library calls never install signal handlers.
    pub cancelled: Arc<AtomicBool>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            base_url: "https://speed.cloudflare.com".into(),
            max_duration: Duration::from_secs(120),
            max_retry_wait: Duration::from_secs(30),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Deadline,
    RetryBudget,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyStatus {
    Disabled,
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct MeasurementError {
    pub status_code: Option<u16>,
    pub reason: String,
}

impl std::fmt::Display for MeasurementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}
impl std::error::Error for MeasurementError {}
impl From<reqwest::Error> for MeasurementError {
    fn from(error: reqwest::Error) -> Self {
        Self {
            status_code: error.status().map(|s| s.as_u16()),
            reason: error.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RunError {
    pub stage: String,
    pub payload_size: Option<usize>,
    #[serde(flatten)]
    pub error: MeasurementError,
}

#[derive(Debug, Serialize)]
pub struct LatencyReport {
    pub status: LatencyStatus,
    pub attempts: u32,
    pub successes: u32,
    pub target_samples: u32,
    pub avg_latency_ms: Option<f64>,
    pub min_latency_ms: Option<f64>,
    pub max_latency_ms: Option<f64>,
    pub latency_measurements: Vec<f64>,
    pub errors: Vec<MeasurementError>,
}

impl LatencyReport {
    pub(crate) fn new(target_samples: u32) -> Self {
        Self {
            status: if target_samples == 0 {
                LatencyStatus::Disabled
            } else {
                LatencyStatus::Failed
            },
            attempts: 0,
            successes: 0,
            target_samples,
            avg_latency_ms: None,
            min_latency_ms: None,
            max_latency_ms: None,
            latency_measurements: vec![],
            errors: vec![],
        }
    }

    pub(crate) fn finish(&mut self) {
        self.successes = self.latency_measurements.len() as u32;
        if self.successes > 0 {
            self.status = if self.successes == self.target_samples {
                LatencyStatus::Complete
            } else {
                LatencyStatus::Partial
            };
            self.avg_latency_ms =
                Some(self.latency_measurements.iter().sum::<f64>() / f64::from(self.successes));
            self.min_latency_ms = self.latency_measurements.iter().copied().reduce(f64::min);
            self.max_latency_ms = self.latency_measurements.iter().copied().reduce(f64::max);
        }
    }
}

pub struct SpeedTestReport {
    pub status: RunStatus,
    pub stop_reason: Option<StopReason>,
    pub metadata: Option<Metadata>,
    pub latency: LatencyReport,
    pub measurements: Vec<Measurement>,
    pub payload_attempt_stats: Vec<PayloadAttemptStats>,
    pub errors: Vec<RunError>,
}

impl SpeedTestReport {
    /// Complete=0, failed=1, partial=3, cancelled=130. Metadata is optional.
    pub fn exit_code(&self) -> u8 {
        if self.stop_reason == Some(StopReason::Cancelled) {
            return 130;
        }
        match self.status {
            RunStatus::Complete => 0,
            RunStatus::Failed => 1,
            RunStatus::Partial => 3,
        }
    }
}

pub(crate) struct RunControl {
    pub config: RunConfig,
    deadline: Instant,
    retry_wait: Duration,
    pub stop_reason: Option<StopReason>,
    pub errors: Vec<RunError>,
}

impl RunControl {
    pub fn new(config: RunConfig) -> Self {
        Self::new_at(config, Instant::now())
    }

    fn new_at(config: RunConfig, now: Instant) -> Self {
        Self {
            deadline: now.checked_add(config.max_duration).unwrap_or(now),
            config,
            retry_wait: Duration::ZERO,
            stop_reason: None,
            errors: vec![],
        }
    }

    fn remaining_at(&mut self, now: Instant) -> Option<Duration> {
        if self.config.cancelled.load(Ordering::SeqCst) {
            self.stop_reason = Some(StopReason::Cancelled);
        }
        if self.stop_reason.is_some() {
            return None;
        }
        let remaining = self.deadline.saturating_duration_since(now);
        if remaining.is_zero() {
            self.stop_reason = Some(StopReason::Deadline);
            None
        } else {
            Some(remaining)
        }
    }

    pub fn remaining(&mut self) -> Option<Duration> {
        self.remaining_at(Instant::now())
    }

    pub fn request_timeout(&mut self) -> Option<Duration> {
        self.remaining().map(|d| d.min(Duration::from_secs(30)))
    }

    fn reserve_retry_at(&mut self, delay: Duration, now: Instant) -> bool {
        let Some(remaining) = self.remaining_at(now) else {
            return false;
        };
        if delay > self.config.max_retry_wait.saturating_sub(self.retry_wait) {
            self.stop_reason = Some(StopReason::RetryBudget);
            return false;
        }
        if delay >= remaining {
            self.stop_reason = Some(StopReason::Deadline);
            return false;
        }
        self.retry_wait += delay;
        true
    }

    pub fn reserve_retry(&mut self, delay: Duration) -> bool {
        self.reserve_retry_at(delay, Instant::now())
    }

    pub fn record(&mut self, stage: &str, payload_size: Option<usize>, error: MeasurementError) {
        self.errors.push(RunError {
            stage: stage.into(),
            payload_size,
            error,
        });
    }
}

pub(crate) fn interruptible_sleep(delay: Duration, cancelled: &AtomicBool) {
    let start = Instant::now();
    while !cancelled.load(Ordering::SeqCst) {
        let remaining = delay.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(remaining.min(Duration::from_millis(20)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_retry_budget_is_cumulative_and_accepts_zero_without_waiting() {
        let now = Instant::now();
        let mut run = RunControl::new_at(
            RunConfig {
                max_retry_wait: Duration::from_secs(3),
                ..RunConfig::default()
            },
            now,
        );
        assert!(run.reserve_retry_at(Duration::from_secs(2), now));
        assert!(run.reserve_retry_at(Duration::from_secs(1), now + Duration::from_secs(2)));
        assert!(run.reserve_retry_at(Duration::ZERO, now + Duration::from_secs(3)));
        assert!(!run.reserve_retry_at(Duration::from_millis(1), now + Duration::from_secs(3)));
        assert_eq!(run.stop_reason, Some(StopReason::RetryBudget));
    }

    #[test]
    fn p1_deadline_does_not_reset_between_stages_or_waits() {
        let now = Instant::now();
        let mut run = RunControl::new_at(
            RunConfig {
                max_duration: Duration::from_secs(5),
                ..RunConfig::default()
            },
            now,
        );
        assert_eq!(
            run.remaining_at(now + Duration::from_secs(4)),
            Some(Duration::from_secs(1))
        );
        assert!(!run.reserve_retry_at(Duration::from_secs(1), now + Duration::from_secs(4)));
        assert_eq!(run.stop_reason, Some(StopReason::Deadline));
        assert!(run.remaining_at(now + Duration::from_secs(5)).is_none());
    }

    #[test]
    fn p1_cancellation_prevents_new_requests_and_retries() {
        let now = Instant::now();
        let mut run = RunControl::new_at(RunConfig::default(), now);
        run.config.cancelled.store(true, Ordering::SeqCst);
        assert!(run.request_timeout().is_none());
        assert!(!run.reserve_retry_at(Duration::ZERO, now));
        assert_eq!(run.stop_reason, Some(StopReason::Cancelled));
    }
}
