use reqwest::Client;
use tracing::warn;

use super::error::SpeedTestError;
use super::types::Metadata;

const TRACE_URL: &str = "https://speed.cloudflare.com/cdn-cgi/trace";

/// Fetch Cloudflare connection metadata from the trace endpoint.
pub async fn fetch_metadata(client: &Client) -> Result<Metadata, SpeedTestError> {
    let resp = client.get(TRACE_URL).send().await?.text().await?;
    parse_trace(&resp)
}

fn parse_trace(body: &str) -> Result<Metadata, SpeedTestError> {
    let mut ip = None;
    let mut colo = None;
    let mut country = None;

    for line in body.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "ip" => ip = Some(value.to_string()),
                "colo" => colo = Some(value.to_string()),
                "loc" => country = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Ok(Metadata {
        ip: ip.unwrap_or_else(|| {
            warn!("Missing 'ip' in trace response");
            "N/A".to_string()
        }),
        colo: colo.unwrap_or_else(|| {
            warn!("Missing 'colo' in trace response");
            "N/A".to_string()
        }),
        country: country.unwrap_or_else(|| {
            warn!("Missing 'loc' in trace response");
            "N/A".to_string()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trace() {
        let body = "fl=123\nip=178.197.1.2\ncolo=ZRH\nloc=CH\nwarp=off\n";
        let meta = parse_trace(body).unwrap();
        assert_eq!(meta.ip, "178.197.1.2");
        assert_eq!(meta.colo, "ZRH");
        assert_eq!(meta.country, "CH");
    }

    #[test]
    fn test_parse_trace_missing_fields() {
        let body = "fl=123\nwarp=off\n";
        let meta = parse_trace(body).unwrap();
        assert_eq!(meta.ip, "N/A");
        assert_eq!(meta.colo, "N/A");
        assert_eq!(meta.country, "N/A");
    }

    #[test]
    fn test_parse_trace_with_ipv6() {
        let body = "ip=2001:db8::1\ncolo=LAX\nloc=US\n";
        let meta = parse_trace(body).unwrap();
        assert_eq!(meta.ip, "2001:db8::1");
    }
}
