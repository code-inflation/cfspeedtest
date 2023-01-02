use reqwest::blocking::Client;

const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";
const UPLOAD_URL: &str = "__up";

fn main() -> Result<(), reqwest::Error> {
    println!("Starting Cloudflare speed test");
    let client = reqwest::blocking::Client::new();
    speed_test(client);
    Ok(())
}

fn speed_test(client: Client) {
    test_downloads(&client);
    test_uploads(&client);
    test_latency(&client);
}

fn test_uploads(client: &Client) {
    test_upload(client, 1024);
}

fn test_downloads(client: &Client) {
    test_download(client, 1024);
}

fn test_latency(client: &Client) {
    for _ in 0..10 {
        test_download(client, 10);
    }
}

fn test_upload(client: &Client, bytes: usize) {
    let url = &format!("{}/{}", BASE_URL, UPLOAD_URL);
    let payload: Vec<u8> = vec![1; bytes];
    let response = client.post(url).body(payload).send().unwrap();
    let status_code = response.status();
    println!("upload with {} bytes -> post: {}", bytes, status_code);
}

fn test_download(client: &Client, bytes: usize) {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, bytes);
    let status_code = client.get(url).send().unwrap().status();
    println!("download with {} bytes -> post: {}", bytes, status_code);
}
