use cfspeedtest::build_client;
use cfspeedtest::engine::download::test_download;
use cfspeedtest::engine::types::PayloadSize;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Testing download speed with 10MB payload");

    let client = build_client(None)?;
    let (mbps, elapsed) = test_download(&client, PayloadSize::M10, None).await?;

    println!("Download speed: {mbps:.1} Mbps (took {elapsed:.2?})");

    Ok(())
}
