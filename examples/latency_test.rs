use cfspeedtest::build_client;
use cfspeedtest::engine::latency::test_latency;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Testing latency (25 samples)");

    let client = build_client(None)?;
    let mut samples = Vec::new();

    for i in 0..25 {
        let rtt_ms = test_latency(&client).await?;
        println!("  Test {}: {rtt_ms:.1} ms", i + 1);
        samples.push(rtt_ms);
    }

    let avg = samples.iter().sum::<f64>() / samples.len() as f64;
    println!("Average latency: {avg:.1} ms");

    Ok(())
}
