use reqwest::Client;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::debug;

use super::error::SpeedTestError;
use super::types::{PayloadSize, SpeedTestEvent, TestType};

const BASE_URL: &str = "https://speed.cloudflare.com";
const UPLOAD_URL: &str = "__up";

/// Run a single upload test.
pub async fn test_upload(
    client: &Client,
    payload_size: PayloadSize,
    tx: Option<&mpsc::Sender<SpeedTestEvent>>,
) -> Result<(f64, std::time::Duration), SpeedTestError> {
    let bytes = payload_size.bytes();
    let url = format!("{BASE_URL}/{UPLOAD_URL}");
    let payload = vec![1u8; bytes];

    // Emit initial progress
    if let Some(tx) = tx {
        let _ = tx.try_send(SpeedTestEvent::TransferProgress {
            test_type: TestType::Upload,
            bytes_so_far: 0,
            total_bytes: bytes as u64,
            current_mbps: 0.0,
        });
    }

    let start = Instant::now();
    let resp = client.post(&url).body(payload).send().await?;
    let elapsed = start.elapsed();

    // Drain response body after timing
    let _ = resp.bytes().await?;

    let mbps = (bytes as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
    debug!("Upload {payload_size}: {mbps:.1} Mbps ({bytes} bytes in {elapsed:.2?})");

    // Emit completion progress
    if let Some(tx) = tx {
        let _ = tx.try_send(SpeedTestEvent::TransferProgress {
            test_type: TestType::Upload,
            bytes_so_far: bytes as u64,
            total_bytes: bytes as u64,
            current_mbps: mbps,
        });
    }

    Ok((mbps, elapsed))
}
