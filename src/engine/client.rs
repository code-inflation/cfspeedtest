use reqwest::Client;
use std::net::IpAddr;
use std::time::Duration;

use super::error::SpeedTestError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build an async reqwest client, optionally bound to a local address.
pub fn build_client(local_addr: Option<IpAddr>) -> Result<Client, SpeedTestError> {
    let mut builder = Client::builder().timeout(REQUEST_TIMEOUT);

    if let Some(addr) = local_addr {
        builder = builder.local_address(addr);
    }

    Ok(builder.build()?)
}
