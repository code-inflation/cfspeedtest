pub mod cli;
pub mod engine;
pub mod output;
pub mod tui;

pub use engine::client::build_client;
pub use engine::error::SpeedTestError;
pub use engine::runner::run_speed_test;
pub use engine::types::*;

/// Run a speed test and return the final result (convenience wrapper).
pub async fn speedtest(
    client: &reqwest::Client,
    config: &SpeedTestConfig,
) -> Result<SpeedTestResult, SpeedTestError> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);

    let client = client.clone();
    let config = config.clone();
    let handle = tokio::spawn(async move { run_speed_test(&client, &config, tx).await });

    // Drain events, return the final result
    while let Some(_event) = rx.recv().await {}

    handle.await.unwrap()
}
