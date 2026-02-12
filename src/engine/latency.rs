use reqwest::Client;
use std::time::Instant;
use tracing::{debug, warn};

use super::error::SpeedTestError;

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";

/// Run a single latency measurement.
///
/// Downloads 0 bytes and measures RTT minus server processing time
/// (from Server-Timing header).
pub async fn test_latency(client: &Client) -> Result<f64, SpeedTestError> {
    let url = format!("{BASE_URL}/{DOWNLOAD_URL}0");
    let start = Instant::now();
    let resp = client.get(&url).send().await?;
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;

    let server_time_ms = resp
        .headers()
        .get("server-timing")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_server_timing)
        .unwrap_or(0.0);

    // Consume body
    let _ = resp.bytes().await?;

    let latency = total_ms - server_time_ms;
    if latency < 0.0 {
        warn!("Negative latency calculated ({latency:.2}ms), clamping to 0");
        Ok(0.0)
    } else {
        debug!("Latency: {latency:.2}ms (total: {total_ms:.2}ms, server: {server_time_ms:.2}ms)");
        Ok(latency)
    }
}

/// Parse `cfRequestDuration;dur=X.XX` from Server-Timing header.
fn parse_server_timing(header: &str) -> Option<f64> {
    header
        .split(';')
        .find_map(|p| p.trim().strip_prefix("dur="))
        .and_then(|v| v.parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_server_timing() {
        assert_eq!(
            parse_server_timing("cfRequestDuration;dur=12.34"),
            Some(12.34)
        );
        assert_eq!(parse_server_timing("cfRequestDuration;dur=0.5"), Some(0.5));
        assert_eq!(parse_server_timing("invalid"), None);
        assert_eq!(parse_server_timing(""), None);
    }
}
