use cfspeedtest::speedtest::speed_test;
use cfspeedtest::speedtest::PayloadSize;
use cfspeedtest::SpeedTestCLIOptions;

fn main() {
    // define speedtest options
    let options = SpeedTestCLIOptions {
        output_format: None,
        ipv4: false,
        ipv6: false,
        verbose: false,
        nr_tests: 5,
        nr_latency_tests: 20,
        max_payload_size: PayloadSize::M10,
    };

    speed_test(reqwest::blocking::Client::new(), options);
}
