use crate::measurements::Measurement;
use crate::speedtest::{
    fetch_metadata, test_download, test_latency, test_upload, PayloadSize, TestType, TIME_THRESHOLD,
};
use crate::tui::app::{LatencyData, SpeedData, TestEvent};
use crate::{OutputFormat, SpeedTestCLIOptions};
use crossbeam_channel::Sender;
use reqwest::blocking::Client;
use std::thread;
use std::time::{Duration, Instant};

pub fn speed_test_tui(
    client: Client,
    options: SpeedTestCLIOptions,
    event_sender: Sender<TestEvent>,
) -> Vec<Measurement> {
    let _metadata = match fetch_metadata(&client) {
        Ok(metadata) => {
            let _ = event_sender.send(TestEvent::MetadataReceived(metadata.clone()));
            metadata
        }
        Err(e) => {
            let _ = event_sender.send(TestEvent::Error(format!("Error fetching metadata: {e}")));
            return Vec::new();
        }
    };

    let mut measurements = Vec::new();

    // Run latency tests
    let (_latency_measurements, _avg_latency) =
        run_latency_test_tui(&client, options.nr_latency_tests, event_sender.clone());

    let payload_sizes = PayloadSize::sizes_from_max(options.max_payload_size.clone());

    // Run download tests
    if options.should_download() {
        measurements.extend(run_tests_tui(
            &client,
            test_download,
            TestType::Download,
            payload_sizes.clone(),
            options.nr_tests,
            options.disable_dynamic_max_payload_size,
            event_sender.clone(),
        ));
    }

    // Run upload tests
    if options.should_upload() {
        measurements.extend(run_tests_tui(
            &client,
            test_upload,
            TestType::Upload,
            payload_sizes.clone(),
            options.nr_tests,
            options.disable_dynamic_max_payload_size,
            event_sender.clone(),
        ));
    }

    let _ = event_sender.send(TestEvent::AllTestsCompleted);
    measurements
}

pub fn run_latency_test_tui(
    client: &Client,
    nr_latency_tests: u32,
    event_sender: Sender<TestEvent>,
) -> (Vec<f64>, f64) {
    let mut measurements: Vec<f64> = Vec::new();

    for _i in 0..=nr_latency_tests {
        let latency = test_latency(client);
        measurements.push(latency);

        let _ = event_sender.send(TestEvent::LatencyMeasurement(LatencyData {
            timestamp: Instant::now(),
            latency,
        }));

        // Small delay to make the UI updates visible
        thread::sleep(Duration::from_millis(50));
    }

    let avg_latency = measurements.iter().sum::<f64>() / measurements.len() as f64;
    (measurements, avg_latency)
}

pub fn run_tests_tui(
    client: &Client,
    test_fn: fn(&Client, usize, OutputFormat) -> f64,
    test_type: TestType,
    payload_sizes: Vec<usize>,
    nr_tests: u32,
    disable_dynamic_max_payload_size: bool,
    event_sender: Sender<TestEvent>,
) -> Vec<Measurement> {
    let mut measurements: Vec<Measurement> = Vec::new();

    for payload_size in payload_sizes {
        let _ = event_sender.send(TestEvent::TestStarted(test_type, payload_size));

        let start = Instant::now();
        for _i in 0..nr_tests {
            let mbit = test_fn(client, payload_size, OutputFormat::None);

            let measurement = Measurement {
                test_type,
                payload_size,
                mbit,
            };
            measurements.push(measurement.clone());

            let _ = event_sender.send(TestEvent::SpeedMeasurement(SpeedData {
                timestamp: Instant::now(),
                speed: mbit,
                test_type,
                payload_size,
            }));

            let _ = event_sender.send(TestEvent::TestCompleted(test_type, payload_size));

            // Small delay to make the UI updates visible
            thread::sleep(Duration::from_millis(100));
        }

        let duration = start.elapsed();

        // Check time threshold for dynamic payload sizing
        if !disable_dynamic_max_payload_size && duration > TIME_THRESHOLD {
            log::info!("Exceeded threshold");
            break;
        }
    }

    measurements
}
