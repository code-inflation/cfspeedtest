mod support;

use serde_json::Value;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use support::{Response, Server};

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{error}: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn p1_cli_invalid_ip_arguments_are_usage_errors() {
    for (flag, value, expected) in [
        ("--ipv4", "not-an-ip", "invalid IPv4 address"),
        ("--ipv6", "not-an-ip", "invalid IPv6 address"),
        // An IPv6 address is not valid for --ipv4 and vice versa.
        ("--ipv4", "::1", "invalid IPv4 address"),
        ("--ipv6", "127.0.0.1", "invalid IPv6 address"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cfspeedtest"))
            .args([flag, value])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "unexpected exit status for {flag} {value}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "missing '{expected}' for {flag} {value}: {stderr}"
        );
        assert!(stderr.contains("Usage:"), "{stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn p1_cli_valid_ip_arguments_are_accepted() {
    let server = Server::new(Response::normal);
    for args in [&["--ipv4"][..], &["--ipv4", "127.0.0.1"]] {
        assert_eq!(server.run(args).status.code(), Some(0));
    }

    // Completion generation validates IPv6 without requiring network access.
    let output = Command::new(env!("CARGO_BIN_EXE_cfspeedtest"))
        .args(["--ipv6", "--generate-completion", "bash"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("cfspeedtest"));
}

#[cfg(target_os = "linux")]
#[test]
fn p1_cli_client_initialization_errors_exit_cleanly() {
    let output = Command::new(env!("CARGO_BIN_EXE_cfspeedtest"))
        .args(["-o", "json"])
        .env("SSL_CERT_FILE", "/dev/null")
        .env("SSL_CERT_DIR", "/dev/null")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("Failed to initialize HTTP client"),
        "{stderr}"
    );
    assert!(stderr.contains("No CA certificates"), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn p1_cli_closed_stdout_preserves_a_successful_run() {
    let server = Server::new(Response::normal);
    for format in ["stdout", "json", "json-pretty", "csv"] {
        let output = run_with_closed_stdout(&mut server.command(&["-o", format]));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{format}: {stderr}");
        assert!(
            !stderr.contains("Failed to write to stdout"),
            "{format}: {stderr}"
        );
    }
}

#[test]
fn p1_cli_closed_stdout_preserves_a_failed_run() {
    let server = Server::new(|path| {
        if path.starts_with("/__down") {
            Response::new(403, "forbidden")
        } else {
            Response::normal(path)
        }
    });
    let output = run_with_closed_stdout(&mut server.command(&[]));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("Run status: Failed"), "{stderr}");
    assert!(!stderr.contains("Failed to write to stdout"), "{stderr}");
}

#[test]
fn p1_cli_help_version_and_completion_survive_closed_stdout() {
    for args in [
        &["--help"][..],
        &["--version"],
        &["--generate-completion", "bash"],
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cfspeedtest"));
        command.args(args);
        let output = run_with_closed_stdout(&mut command);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {stderr}");
        assert!(
            !stderr.contains("Failed to write to stdout"),
            "{args:?}: {stderr}"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn p1_cli_reports_other_stdout_errors_once() {
    let server = Server::new(Response::normal);
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = server
        .command(&["-o", "stdout"])
        .stdout(full)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert_eq!(
        stderr.matches("Failed to write to stdout").count(),
        1,
        "{stderr}"
    );
}

fn run_with_closed_stdout(command: &mut Command) -> Output {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let child = command
        .stdout(writer)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    support::finish(child, Duration::from_secs(4))
}

#[test]
fn p1_cli_all_speed_samples_failed_is_nonzero() {
    let server = Server::new(|path| {
        if path.starts_with("/__down") {
            Response::new(403, "forbidden")
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&[]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["status"], "failed");
    assert_eq!(report["speed_measurements"][0]["successes"], 0);
    assert_eq!(report["errors"][0]["status_code"], 403);
    assert!(!output.stderr.is_empty());
}

#[test]
fn p1_cli_latency_http_errors_are_not_samples() {
    let server = Server::new(|path| {
        if path == "/__down?bytes=0" {
            Response::new(503, "unavailable")
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&["--nr-latency-tests", "2"]);
    let report = json(&output);
    assert_eq!(report["latency_measurement"]["successes"], 0);
    assert_eq!(report["latency_measurement"]["attempts"], 2);
    assert_eq!(report["latency_measurement"]["status"], "failed");
    assert!(report["latency_measurement"]["avg_latency_ms"].is_null());
    assert_eq!(report["status"], "partial");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn p1_cli_failed_latency_is_na_in_human_output() {
    let server = Server::new(|path| {
        let mut response = Response::normal(path);
        response.disconnect = path == "/__down?bytes=0";
        response
    });
    let output = server.run(&["--nr-latency-tests", "2", "-o", "stdout"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("latency N/A"), "{text}");
    assert!(!text.contains("latency 0.00"));
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn p1_cli_metadata_errors_are_optional_and_explicit() {
    for (status, body, disconnect) in [
        (503, "unavailable", false),
        (200, "ip=not-an-ip\ncolo=\n", false),
        (200, "", true),
    ] {
        let server = Server::new(move |path| {
            if path == "/cdn-cgi/trace" {
                let mut r = Response::new(status, body);
                r.disconnect = disconnect;
                r
            } else {
                Response::normal(path)
            }
        });
        let output = server.run(&[]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = json(&output);
        assert!(report["metadata"].is_null(), "{report}");
        assert_eq!(report["status"], "complete");
        assert_eq!(report["errors"][0]["stage"], "metadata");
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn p1_cli_partial_samples_have_distinct_exit_and_keep_results() {
    let count = AtomicUsize::new(0);
    let server = Server::new(move |path| {
        if path.starts_with("/__down") && count.fetch_add(1, Ordering::SeqCst) > 0 {
            Response::new(403, "forbidden")
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&["-n", "2"]);
    assert_eq!(output.status.code(), Some(3));
    let report = json(&output);
    assert_eq!(report["status"], "partial");
    assert_eq!(report["speed_measurements"][0]["successes"], 1);
    assert!(report["speed_measurements"][0]["avg"].as_f64().unwrap() > 0.0);
}

#[test]
fn p1_cli_success_and_disabled_latency_are_explicit() {
    let server = Server::new(Response::normal);
    let output = server.run(&[]);
    assert_eq!(output.status.code(), Some(0));
    let report = json(&output);
    assert_eq!(report["status"], "complete");
    assert_eq!(report["latency_measurement"]["status"], "disabled");
    assert_eq!(report["latency_measurement"]["attempts"], 0);
}

#[test]
fn p1_cli_valid_zero_latency_is_preserved() {
    let server = Server::new(|path| {
        let mut response = Response::normal(path);
        if path == "/__down?bytes=0" {
            response
                .headers
                .push(("Server-Timing".into(), "cfL4;desc=\"?rtt=0\"".into()));
        }
        response
    });
    let output = server.run(&["--nr-latency-tests", "1"]);
    let report = json(&output);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(report["latency_measurement"]["status"], "complete");
    assert_eq!(report["latency_measurement"]["successes"], 1);
    assert_eq!(report["latency_measurement"]["avg_latency_ms"], 0.0);
}

#[test]
fn p1_cli_refuses_retry_after_beyond_budget() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let server = Server::new(move |path| {
        if path.starts_with("/__down") {
            observed.fetch_add(1, Ordering::SeqCst);
            let mut r = Response::new(429, "retry later");
            r.headers.push(("Retry-After".into(), "86400".into()));
            r
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&["--max-retry-wait", "1"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(json(&output)["stop_reason"], "retry_budget");
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn p1_cli_deadline_bounds_body_reads() {
    let server = Server::new(|path| {
        let mut r = Response::normal(path);
        if path.starts_with("/__down") {
            // The fixture's watchdog fails the test if the CLI waits for the body.
            r.delay = Duration::MAX;
        }
        r
    });
    let output = server.run(&["--max-duration", "1"]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["stop_reason"], "deadline");
    assert_eq!(report["speed_measurements"][0]["successes"], 0);
}

#[test]
fn p1_cli_upload_body_timeout_is_not_success() {
    let server = Server::new(|path| {
        let mut response = Response::normal(path);
        if path == "/__up" {
            response.delay = Duration::MAX;
        }
        response
    });
    let output = server.run(&["--upload-only", "--max-duration", "1"]);
    let report = json(&output);
    assert_eq!(report["speed_measurements"][0]["successes"], 0, "{report}");
    assert_eq!(report["stop_reason"], "deadline");
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn p1_cli_metadata_body_is_covered_by_overall_deadline() {
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = requests.clone();
    let server = Server::new(move |path| {
        observed.fetch_add(1, Ordering::SeqCst);
        let mut r = Response::normal(path);
        if path == "/cdn-cgi/trace" {
            r.delay = Duration::MAX;
        }
        r
    });
    let output = server.run(&["--max-duration", "1"]);
    assert_eq!(output.status.code(), Some(1));
    let report = json(&output);
    assert_eq!(report["stop_reason"], "deadline");
    assert!(report["metadata"].is_null());
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[test]
fn p1_cli_latency_rejects_truncated_body_and_retains_valid_samples() {
    let latency_count = AtomicUsize::new(0);
    let server = Server::new(move |path| {
        let mut response = Response::normal(path);
        if path == "/__down?bytes=0" && latency_count.fetch_add(1, Ordering::SeqCst) == 0 {
            response.body = b"x".to_vec();
            response.declared_length = Some(4);
        }
        response
    });
    let output = server.run(&["--nr-latency-tests", "2"]);
    let report = json(&output);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(report["latency_measurement"]["attempts"], 2);
    assert_eq!(report["latency_measurement"]["successes"], 1);
    assert_eq!(
        report["latency_measurement"]["errors"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(report["latency_measurement"]["status"], "partial");
    assert!(
        report["latency_measurement"]["avg_latency_ms"]
            .as_f64()
            .unwrap()
            >= 0.0
    );
}

#[test]
fn p1_cli_recovered_retry_is_complete_and_keeps_error_history() {
    let count = AtomicUsize::new(0);
    let server = Server::new(move |path| {
        if path.starts_with("/__down") && count.fetch_add(1, Ordering::SeqCst) == 0 {
            let mut response = Response::new(429, "retry");
            response.headers.push(("Retry-After".into(), "0".into()));
            response
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&[]);
    let report = json(&output);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(report["status"], "complete");
    assert_eq!(report["speed_measurements"][0]["attempts"], 2);
    assert_eq!(report["speed_measurements"][0]["successes"], 1);
    assert_eq!(report["errors"][0]["status_code"], 429);
}

#[test]
fn p1_cli_csv_failures_keep_stdout_parseable() {
    let server = Server::new(|path| {
        if path.starts_with("/__down") {
            Response::new(403, "forbidden")
        } else {
            Response::normal(path)
        }
    });
    let output = server.run(&["-o", "csv"]);
    assert_eq!(output.status.code(), Some(1));
    let mut reader = csv::Reader::from_reader(output.stdout.as_slice());
    assert_eq!(reader.headers().unwrap().get(0), Some("test_type"));
    let success_column = reader
        .headers()
        .unwrap()
        .iter()
        .position(|name| name == "successes")
        .unwrap();
    let records: Vec<_> = reader.records().collect::<Result<_, _>>().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].get(success_column), Some("0"));
    assert!(!output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn p1_cli_cancellation_preserves_completed_samples() {
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let server = Server::new(move |path| {
        if path.starts_with("/__down") && observed.fetch_add(1, Ordering::SeqCst) > 0 {
            let mut r = Response::new(429, "retry later");
            r.headers.push(("Retry-After".into(), "2".into()));
            r
        } else {
            Response::normal(path)
        }
    });
    let mut child = server.command(&["-n", "2"]).spawn().unwrap();
    let start = Instant::now();
    while count.load(Ordering::SeqCst) < 2 {
        if start.elapsed() > Duration::from_secs(2) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI did not reach retry");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let output = support::finish(child, Duration::from_secs(1));
    assert_eq!(output.status.code(), Some(130));
    let report = json(&output);
    assert_eq!(report["stop_reason"], "cancelled");
    assert_eq!(report["speed_measurements"][0]["successes"], 1);
}
