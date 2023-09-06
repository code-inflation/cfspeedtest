use cfspeedtest::speedtest::test_download;
use cfspeedtest::OutputFormat;

fn main() {
    println!("Testing download speed with 10MB of payload");

    let download_speed = test_download(
        &reqwest::blocking::Client::new(),
        10_000_000,
        OutputFormat::None, // don't write to stdout while running the test
    );

    println!("download speed in mbit: {download_speed}")
}
