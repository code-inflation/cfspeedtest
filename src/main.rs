use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    StatusCode,
};
use std::time::{Duration, Instant};

const BASE_URL: &str = "http://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";
const NR_TEST_RUNS: u32 = 1;

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
    for _ in 0..NR_TEST_RUNS {
        test_upload(client, 100_000);
    }
    for _ in 0..NR_TEST_RUNS {
        test_upload(client, 1_000_000);
    }
    for _ in 0..NR_TEST_RUNS {
        test_upload(client, 10_000_000);
    }
}

fn test_downloads(client: &Client) {
    for _ in 0..NR_TEST_RUNS {
        test_download(client, 100_000);
    }
    for _ in 0..NR_TEST_RUNS {
        test_download(client, 1_000_000);
    }
    for _ in 0..NR_TEST_RUNS {
        test_download(client, 10_000_000);
    }
    for _ in 0..NR_TEST_RUNS {
        test_download(client, 100_000_000);
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
    let req_builder = client.post(url).body(payload);
    let (status_code, mbits, duration) = timed_send(req_builder, bytes);
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
    let req_builder = client.get(url);
    let (status_code, mbits, duration) = timed_send(req_builder, bytes);
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

fn timed_send(req_builder: RequestBuilder, bytes: usize) -> (StatusCode, f64, Duration) {
    let start = Instant::now();
    let response = req_builder.send().unwrap();
    let status_code = response.status();
    let _res_bytes = response.bytes();
    let duration = start.elapsed();
    let mbits = (bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
    (status_code, mbits, duration)
}
