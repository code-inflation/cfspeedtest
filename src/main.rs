const BASE_URL: &str = "https://speed.cloudflare.com";
const DOWNLOAD_URL: &str = "__down?bytes=";

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    println!("Starting Cloudflare speed test");
    speed_test().await?;
    Ok(())
}

async fn speed_test() -> Result<(), reqwest::Error> {
    test_download().await?;
    test_upload().await?;
    test_latency().await?;
    Ok(())
}

async fn test_latency() -> Result<(), reqwest::Error> {
    todo!()
}

async fn test_upload() -> Result<(), reqwest::Error> {
    todo!()
}

async fn test_download() -> Result<(), reqwest::Error> {
    let url = &format!("{}/{}{}", BASE_URL, DOWNLOAD_URL, 1024);
    let body = reqwest::get(url).await?;
    println!("{:?}", body.text().await?);
    Ok(())
}
