use crate::engine::types::SpeedTestResult;

pub fn print_csv(result: &SpeedTestResult) {
    let mut wtr = csv::Writer::from_writer(std::io::stdout());

    wtr.write_record([
        "test_type",
        "payload_size",
        "min",
        "q1",
        "median",
        "q3",
        "max",
        "avg",
    ])
    .unwrap();

    if let Some(ref lat) = result.latency {
        wtr.write_record([
            "latency",
            "0",
            &format!("{:.2}", lat.min_ms),
            "", // no quartiles for latency
            &format!("{:.2}", lat.avg_ms),
            "",
            &format!("{:.2}", lat.max_ms),
            &format!("{:.2}", lat.avg_ms),
        ])
        .unwrap();
    }

    for throughput in [&result.download, &result.upload].into_iter().flatten() {
        for stat in &throughput.stats {
            wtr.write_record([
                &stat.test_type.to_string().to_lowercase(),
                &stat.payload_size.bytes().to_string(),
                &format!("{:.2}", stat.min),
                &format!("{:.2}", stat.q1),
                &format!("{:.2}", stat.median),
                &format!("{:.2}", stat.q3),
                &format!("{:.2}", stat.max),
                &format!("{:.2}", stat.avg),
            ])
            .unwrap();
        }
    }

    wtr.flush().unwrap();
}
