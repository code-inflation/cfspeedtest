use cfspeedtest::speedtest::speed_test;
use cfspeedtest::speedtest::PayloadSize;
use cfspeedtest::OutputFormat;
use cfspeedtest::SpeedTestCLIOptions;

fn main() {
    // define speedtest options
    let options = SpeedTestCLIOptions {
        output_format: OutputFormat::None, // don't write to stdout
        ipv4: false,                       // don't force ipv4 usage
        ipv6: false,                       // don't force ipv6 usage
        verbose: false,
        nr_tests: 5,
        nr_latency_tests: 20,
        max_payload_size: PayloadSize::M10,
    };

    let measurements = speed_test(reqwest::blocking::Client::new(), options);
    measurements
        .iter()
        .for_each(|measurement| println!("{measurement}"));
}
