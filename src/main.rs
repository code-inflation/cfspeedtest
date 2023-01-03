use reqwest::{
    blocking::{Client, RequestBuilder},
    header::HeaderValue,
    StatusCode,
};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

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
    let metadata = fetch_metadata(&client);
    println!("{}", metadata);
    test_latency(&client);
    test_downloads(&client);
    test_uploads(&client);
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
    // for _ in 0..NR_TEST_RUNS {
    //     test_download(client, 100_000_000);
    // }
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
    let response = req_builder.send().expect("failed to get response");
    let status_code = response.status();
    let _res_bytes = response.bytes();
    let duration = start.elapsed();
    let mbits = (bytes as f64 * 8.0 / 1_000_000.0) / duration.as_secs_f64();
    (status_code, mbits, duration)
}

struct Metadata {
    city: String,
    country: String,
    ip: String,
    asn: String,
    colo: String,
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "City: {}\nCountry: {}\nIp: {}\nAsn: {}\nColo: {}",
            self.city, self.country, self.ip, self.asn, self.colo
        )
    }
}

fn fetch_metadata(client: &Client) -> Metadata {
    // TODO fix this mess
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, 0);
    let headers = client
        .get(url)
        .send()
        .expect("failed to get response")
        .headers()
        .to_owned();

    let city = headers
        .get("cf-meta-city")
        .unwrap_or(&HeaderValue::from_str("City N/A").unwrap())
        .to_str()
        .unwrap()
        .to_owned();
    let country = headers
        .get("cf-meta-country")
        .unwrap_or(&HeaderValue::from_str("Country N/A").unwrap())
        .to_str()
        .unwrap()
        .to_owned();
    let ip = headers
        .get("cf-meta-ip")
        .unwrap_or(&HeaderValue::from_str("IP N/A").unwrap())
        .to_str()
        .unwrap()
        .to_owned();
    let asn = headers
        .get("cf-meta-asn")
        .unwrap_or(&HeaderValue::from_str("ASN N/A").unwrap())
        .to_str()
        .unwrap()
        .to_owned();
    let colo = headers
        .get("cf-meta-colo")
        .unwrap_or(&HeaderValue::from_str("Colo N/A").unwrap())
        .to_str()
        .unwrap()
        .to_owned();

    Metadata {
        city,
        country,
        ip,
        asn,
        colo,
    }
}
