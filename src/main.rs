pub mod measurements;
pub mod progress;
pub mod speedtest;

use speedtest::speed_test;

fn main() {
    println!("Starting Cloudflare speed test");
    let client = reqwest::blocking::Client::new();
    speed_test(client);
}
