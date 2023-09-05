use cfspeedtest::speedtest::run_latency_test;
use cfspeedtest::OutputFormat;

fn main() {
    println!("Testing latency");

    let (latency_results, avg_latency) = run_latency_test(
        &reqwest::blocking::Client::new(),
        25,
        Some(OutputFormat::Json),
    );

    println!("average latancy in ms: {avg_latency}");

    println!("all latency test results");
    for latency_result in latency_results {
        println!("latency in ms: {latency_result}");
    }
}
