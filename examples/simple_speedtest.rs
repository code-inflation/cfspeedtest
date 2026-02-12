use cfspeedtest::engine::types::{PayloadSize, SpeedTestConfig};
use cfspeedtest::{build_client, speedtest};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = build_client(None)?;

    let config = SpeedTestConfig {
        nr_tests: 5,
        nr_latency_tests: 20,
        max_payload_size: PayloadSize::M10,
        ..Default::default()
    };

    let result = speedtest(&client, &config).await?;

    if let Some(ref dl) = result.download {
        println!("Download: {:.1} Mbps", dl.overall_mbps);
    }
    if let Some(ref ul) = result.upload {
        println!("Upload: {:.1} Mbps", ul.overall_mbps);
    }
    if let Some(ref lat) = result.latency {
        println!("Latency: {:.1} ms", lat.avg_ms);
    }

    Ok(())
}
