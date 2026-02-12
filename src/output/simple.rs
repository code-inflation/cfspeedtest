use crate::engine::types::SpeedTestResult;

/// Print a one-line summary: "↓ 450 Mbps  ↑ 120 Mbps  ⏱ 12ms"
pub fn print_simple(result: &SpeedTestResult) {
    let mut parts = Vec::new();

    if let Some(ref dl) = result.download {
        parts.push(format!("↓ {:.1} Mbps", dl.overall_mbps));
    }
    if let Some(ref ul) = result.upload {
        parts.push(format!("↑ {:.1} Mbps", ul.overall_mbps));
    }
    if let Some(ref lat) = result.latency {
        parts.push(format!("⏱ {:.1}ms", lat.avg_ms));
    }

    println!("{}", parts.join("  "));
}
