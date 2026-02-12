use futures::StreamExt;
use reqwest::Client;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::debug;

use super::error::SpeedTestError;
use super::types::{PayloadSize, SpeedTestEvent, TestType};

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";

/// Run a single download test, optionally emitting progress events.
pub async fn test_download(
    client: &Client,
    payload_size: PayloadSize,
    tx: Option<&mpsc::Sender<SpeedTestEvent>>,
) -> Result<(f64, std::time::Duration), SpeedTestError> {
    let bytes = payload_size.bytes();
    let url = format!("{BASE_URL}/{DOWNLOAD_URL}{bytes}");

    let start = Instant::now();
    let resp = client.get(&url).send().await?;

    let mut total_bytes: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total_bytes += chunk.len() as u64;

        if let Some(tx) = tx {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let current_mbps = (total_bytes as f64 * 8.0) / (elapsed * 1_000_000.0);
                let _ = tx.try_send(SpeedTestEvent::TransferProgress {
                    test_type: TestType::Download,
                    bytes_so_far: total_bytes,
                    total_bytes: bytes as u64,
                    current_mbps,
                });
            }
        }
    }

    let elapsed = start.elapsed();
    let mbps = (total_bytes as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
    debug!("Download {payload_size}: {mbps:.1} Mbps ({total_bytes} bytes in {elapsed:.2?})");

    Ok((mbps, elapsed))
}
