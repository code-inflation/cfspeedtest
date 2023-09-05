use cfspeedtest::speedtest::test_download;
use cfspeedtest::OutputFormat;

fn main() {
    println!("Testing download speed with 10MB of payload");

    let download_speed = test_download(
        &reqwest::blocking::Client::new(),
        10_000_000,
        Some(OutputFormat::Json),
    );

    println!("download speed in mbit: {download_speed}")
}
