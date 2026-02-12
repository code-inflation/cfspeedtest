use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info};

use super::download::test_download;
use super::error::SpeedTestError;
use super::latency::test_latency;
use super::metadata::fetch_metadata;
use super::types::*;
use super::upload::test_upload;

const TIME_THRESHOLD: Duration = Duration::from_secs(5);

/// Run the full speed test suite, emitting events as progress is made.
pub async fn run_speed_test(
    client: &reqwest::Client,
    config: &SpeedTestConfig,
    tx: mpsc::Sender<SpeedTestEvent>,
) -> Result<SpeedTestResult, SpeedTestError> {
    // 1. Fetch metadata
    info!("Fetching server metadata...");
    let metadata = fetch_metadata(client).await?;
    let _ = tx
        .send(SpeedTestEvent::MetadataReady(metadata.clone()))
        .await;

    // 2. Latency tests
    info!("Running {} latency tests...", config.nr_latency_tests);
    let latency = run_latency_tests(client, config.nr_latency_tests, &tx).await?;

    // 3. Download tests
    let download = if config.download {
        let _ = tx
            .send(SpeedTestEvent::PhaseStart(TestType::Download))
            .await;
        Some(run_throughput_tests(client, config, TestType::Download, &tx).await?)
    } else {
        None
    };

    // 4. Upload tests
    let upload = if config.upload {
        let _ = tx.send(SpeedTestEvent::PhaseStart(TestType::Upload)).await;
        Some(run_throughput_tests(client, config, TestType::Upload, &tx).await?)
    } else {
        None
    };

    let result = SpeedTestResult {
        metadata,
        latency: Some(latency),
        download,
        upload,
    };

    let _ = tx.send(SpeedTestEvent::Complete(result.clone())).await;
    Ok(result)
}

async fn run_latency_tests(
    client: &reqwest::Client,
    count: u32,
    tx: &mpsc::Sender<SpeedTestEvent>,
) -> Result<LatencyResult, SpeedTestError> {
    let mut samples = Vec::with_capacity(count as usize);

    for i in 0..count {
        match test_latency(client).await {
            Ok(rtt_ms) => {
                samples.push(rtt_ms);
                let _ = tx
                    .send(SpeedTestEvent::LatencySample {
                        rtt_ms,
                        index: i + 1,
                        total: count,
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(SpeedTestEvent::Error(format!(
                        "Latency test {}: {e}",
                        i + 1
                    )))
                    .await;
            }
        }
    }

    let avg = if samples.is_empty() {
        0.0
    } else {
        samples.iter().sum::<f64>() / samples.len() as f64
    };

    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let result = LatencyResult {
        avg_ms: avg,
        min_ms: if min.is_infinite() { 0.0 } else { min },
        max_ms: if max.is_infinite() { 0.0 } else { max },
        samples,
    };

    let _ = tx
        .send(SpeedTestEvent::LatencyComplete(result.clone()))
        .await;
    Ok(result)
}

async fn run_throughput_tests(
    client: &reqwest::Client,
    config: &SpeedTestConfig,
    test_type: TestType,
    tx: &mpsc::Sender<SpeedTestEvent>,
) -> Result<ThroughputResult, SpeedTestError> {
    let payload_sizes = PayloadSize::sizes_up_to(config.max_payload_size);
    let mut all_measurements = Vec::new();
    let mut all_stats = Vec::new();
    let mut skip_remaining = false;

    let total_tests_per_size = config.nr_tests;

    for &payload_size in &payload_sizes {
        if skip_remaining {
            let _ = tx
                .send(SpeedTestEvent::PayloadSkipped {
                    test_type,
                    payload_size,
                })
                .await;
            continue;
        }

        let mut size_measurements = Vec::new();

        for i in 0..total_tests_per_size {
            let (mbps, elapsed) = match test_type {
                TestType::Download => test_download(client, payload_size, Some(tx)).await?,
                TestType::Upload => test_upload(client, payload_size, Some(tx)).await?,
            };

            size_measurements.push(mbps);
            all_measurements.push(Measurement {
                test_type,
                payload_size,
                mbps,
            });

            let _ = tx
                .send(SpeedTestEvent::ThroughputSample {
                    test_type,
                    payload_size,
                    mbps,
                    index: i + 1,
                    total: total_tests_per_size,
                })
                .await;

            if !config.disable_dynamic_max_payload_size && elapsed > TIME_THRESHOLD {
                debug!("{test_type} {payload_size} took {elapsed:.2?}, skipping larger payloads");
                skip_remaining = true;
                break;
            }
        }

        if !size_measurements.is_empty() {
            let (min, q1, median, q3, max, avg) = calc_stats(&size_measurements);
            all_stats.push(PayloadStats {
                test_type,
                payload_size,
                min,
                q1,
                median,
                q3,
                max,
                avg,
            });
        }
    }

    // Overall speed = avg of the largest payload size tested
    let overall_mbps = all_stats.last().map(|s| s.avg).unwrap_or(0.0);

    let result = ThroughputResult {
        overall_mbps,
        measurements: all_measurements,
        stats: all_stats,
    };

    let _ = tx
        .send(SpeedTestEvent::ThroughputComplete {
            test_type,
            result: result.clone(),
        })
        .await;

    Ok(result)
}
