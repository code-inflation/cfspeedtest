use reqwest::blocking::Client;
use std::time::Instant;

const BASE_URL: &str = "http://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";

fn main() {
    println!("Starting Cloudflare speed test");
    let client = reqwest::blocking::Client::new();
    speed_test(client);
}

fn speed_test(client: Client) {
    test_latency(&client);
    // fetch_server_loc_data();
    // fetch_cdncgi_trace();
    test_downloads(&client);
    test_uploads(&client);
}

fn fetch_cdncgi_trace() {
    todo!()
}

fn fetch_server_loc_data() {
    todo!()
}

fn print_boxplot() {
    todo!()
}

fn test_uploads(client: &Client) {
    for _ in 0..10 {
        test_upload(client, 100_000);
    }
    for _ in 0..10 {
        test_upload(client, 1_000_000);
    }
}

fn test_downloads(client: &Client) {
    for _ in 0..10 {
        test_download(client, 100_000);
    }
    for _ in 0..10 {
        test_download(client, 1_000_000);
    }
}

fn test_latency(client: &Client) {
    // TODO measure time to first byte - server processing time
    // for _ in 0..10 {
    //     test_download(client, 1);
    // }
}

fn test_upload(client: &Client, bytes: usize) -> f64 {
    let url = &format!("{}/{}", BASE_URL, UPLOAD_URL);
    let payload: Vec<u8> = vec![1; bytes];
    let start = Instant::now();
    let response = client.post(url).body(payload).send().unwrap();
    let status_code = response.status();
    let duration = start.elapsed();
    let mbit = bytes as f64 * 8.0 / 1_000_000.0;
    let mbits = mbit / duration.as_secs_f64();
    println!(
        "upload {:.2} mbit/s with {} in {}ms -> post: {}",
        mbits,
        format_bytes(bytes),
        duration.as_millis(),
        status_code
    );
    mbits
}

fn test_download(client: &Client, bytes: usize) -> f64 {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, bytes);
    let start = Instant::now();
    let response = client.get(url).send().unwrap();
    let status_code = response.status();
    response.text().unwrap();
    let duration = start.elapsed();
    let mbit = bytes as f64 * 8.0 / 1_000_000.0;
    let mbits = mbit / duration.as_secs_f64();
    println!(
        "download {:.2} mbit/s with {} in {}ms -> get: {}",
        mbits,
        format_bytes(bytes),
        duration.as_millis(),
        status_code
    );
    mbits
}

fn format_bytes(bytes: usize) -> String {
    match bytes {
        1_000..=999_999 => format!("{}KB", bytes / 1_000),
        1_000_000..=999_999_999 => format!("{}MB", bytes / 1_000_000),
        _ => format!("{} bytes", bytes),
    }
}
