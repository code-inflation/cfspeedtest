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
    test_download(&client);
    test_upload(&client);
    test_latency(&client);
}

fn test_latency(client: &Client) {
    for _ in 0..10 {
        test_download(client);
    }
}

fn test_upload(client: &Client) {
    let url = &format!("{}/{}", BASE_URL, UPLOAD_URL);
    let response = client.post(url).body("test body").send().unwrap();
    let status_code = response.status();
    println!("post: {}", status_code);
}

fn test_download(client: &Client) {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, 1024);
    let status_code = client.get(url).send().unwrap().status();
    println!("get: {}", status_code);
}
