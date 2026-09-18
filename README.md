# cfspeedtest - Unofficial CLI for [speed.cloudflare.com](https://speed.cloudflare.com)
![CI](https://github.com/code-inflation/cfspeedtest/actions/workflows/CI.yml/badge.svg)
![Release](https://github.com/code-inflation/cfspeedtest/actions/workflows/release.yaml/badge.svg)
![Crates.io Version](https://img.shields.io/crates/v/cfspeedtest)
![Crates.io Downloads](https://img.shields.io/crates/d/cfspeedtest?label=Crates.io%20downloads)


## Installation
Install using `cargo`:
```sh
cargo install cfspeedtest
```

Or download the latest binary release here: [cfspeedtest/releases/latest](https://github.com/code-inflation/cfspeedtest/releases/latest)

Alternatively there is also a [docker image available on dockerhub](https://hub.docker.com/r/cybuerg/cfspeedtest)
```sh
docker run cybuerg/cfspeedtest
```

## Usage
```
> cfspeedtest --help
Unofficial CLI for speed.cloudflare.com

Usage: cfspeedtest [OPTIONS]

Options:
  -n, --nr-tests <NR_TESTS>
          Number of test runs per payload size [default: 10]
      --nr-latency-tests <NR_LATENCY_TESTS>
          Number of latency tests to run [default: 25]
  -m, --max-payload-size <MAX_PAYLOAD_SIZE>
          The max payload size in bytes to use [100k, 1m, 10m, 25m or 100m] [default: 25MB]
  -o, --output-format <OUTPUT_FORMAT>
          Set the output format [csv, json or json-pretty] > This silences all other output to stdout [default: StdOut]
  -v, --verbose
          Enable verbose output i.e. print boxplots of the measurements
      --ipv4 [<IPv4>]
          Force IPv4 with provided source IPv4 address or the default IPv4 address bound to the main interface
      --ipv6 [<IPv6>]
          Force IPv6 with provided source IPv6 address or the default IPv6 address bound to the main interface
  -d, --disable-dynamic-max-payload-size
          Disables dynamically skipping tests with larger payload sizes if the tests for the previous payload size took longer than 5 seconds
      --download-only
          Test download speed only
      --upload-only
          Test upload speed only
      --generate-completion <COMPLETION>
          Generate shell completion script for the specified shell [possible values: bash, elvish, fish, powershell, zsh]
      --server <SERVER>
          Base URL of a compatible Cloudflare speed-test service [default: https://speed.cloudflare.com]
      --max-duration <MAX_DURATION>
          Maximum total run duration in seconds [default: 120]
      --max-retry-wait <MAX_RETRY_WAIT>
          Maximum cumulative retry wait in seconds [default: 30]
  -h, --help
          Print help
  -V, --version
          Print version
```

Example usage:  
[![asciicast](https://asciinema.org/a/Moun5mFB1sm1VFkkFljG9UGyz.svg)](https://asciinema.org/a/Moun5mFB1sm1VFkkFljG9UGyz)

Example with json-pretty output:  
[![asciicast](https://asciinema.org/a/P6IUAADtaCq3bT18GbYVHmksA.svg)](https://asciinema.org/a/P6IUAADtaCq3bT18GbYVHmksA)

### Run limits and failures

Runs have a shared 120-second measurement deadline and a 30-second cumulative retry-wait budget by default. Both limits cover the entire run, including metadata, latency, and both transfer directions. Each request is limited to the smaller of 30 seconds and the remaining run time.

```sh
cfspeedtest --max-duration 60 --max-retry-wait 10 -o json
```

`--max-duration` and `--max-retry-wait` are in seconds. Setting the retry-wait budget to zero prevents positive retry waits. The client accepts both seconds and HTTP dates in `Retry-After`; when the requested delay exceeds either remaining budget, it stops and retains completed measurements instead of retrying early. `--disable-dynamic-max-payload-size` does not disable these limits.

Ctrl-C stops new measurements and interrupts retry waits. An in-flight blocking request may take up to its remaining request timeout to finish; completed samples are retained. On Unix, SIGTERM and SIGHUP use the same graceful cancellation path.

| Exit code | Meaning |
| --- | --- |
| 0 | Complete: every attempted payload reached its successful-sample target and all enabled latency probes succeeded. Payloads omitted by normal adaptive stopping do not make a run partial. |
| 1 | Failed: no valid throughput samples were collected, or CLI initialization failed. |
| 2 | Invalid command-line arguments. |
| 3 | Partial: some valid throughput samples exist, but a sample target was missed or a run limit stopped testing. |
| 130 | Cancelled; any completed measurements are retained. |

Metadata is optional: an unavailable or invalid trace response is reported as an error, but does not by itself make a successful measurement run fail. Failed samples that are replaced by successful retries remain in the error history without making a completed run partial. Operational diagnostics go to stderr in every CLI output mode.

JSON includes `status`, `stop_reason` (`deadline`, `retry_budget`, `cancelled`, or null), and structured `errors` with stage, payload size, HTTP status when available, and reason. Missing metadata is null. `latency_measurement` is always present and includes `status`, `attempts`, `successes`, `target_samples`, and `errors`. Its summary values are null when no valid samples exist; human output uses N/A. `--nr-latency-tests 0` explicitly disables latency testing. CSV retains its throughput-only columns; use the exit code and stderr to detect failures, or JSON for the full outcome.

Downloads must deliver exactly the requested number of bytes without body-read errors. Upload response-body errors also invalidate the sample, while upload timing still stops at response headers. Quartiles use the median of each half, excluding the middle observation for odd sample counts; a singleton uses that observation for every summary statistic.

`--server <URL>` selects a compatible service exposing `/cdn-cgi/trace`, `/__down?bytes=...`, and `/__up`. It defaults to `https://speed.cloudflare.com` and also supports local HTTP fixtures for testing.

### Library outcomes

`speed_test_with_config(client, options, RunConfig)` returns a `SpeedTestReport` with metadata, latency, throughput samples, attempt counts, errors, and an exit-code policy. Set `options.output_format = OutputFormat::None` to consume it without printing. `RunConfig` supplies the endpoint, time budgets, and a shared cancellation flag; library calls never install signal handlers or exit the host process.

`run_latency_report` and `try_test_latency` provide explicit latency outcomes. The existing `speed_test` and tuple/floating-point functions remain available. Missing latency from `test_latency` or `run_latency_test` now uses NaN instead of the misleading zero fallback. `fetch_metadata` returns `MeasurementError` so it can report invalid metadata as well as HTTP/transport errors.

### Shell Completion

`cfspeedtest` supports generating shell completion scripts. Use the `--generate-completion` flag followed by your shell name (e.g., `bash`, `zsh`, `fish`, `powershell`, `elvish`).

Example for bash (add to `~/.bashrc` or similar):
```sh
cfspeedtest --generate-completion bash > ~/.local/share/bash-completion/completions/cfspeedtest
# Or, if you don't have a completions directory set up:
# source <(cfspeedtest --generate-completion bash)
```

Example for zsh (add to `~/.zshrc` or similar):
```sh
# Ensure your fpath includes a directory for completions, e.g., ~/.zfunc
# mkdir -p ~/.zfunc
# echo 'fpath=(~/.zfunc $fpath)' >> ~/.zshrc
cfspeedtest --generate-completion zsh > ~/.zfunc/_cfspeedtest
# You may need to run compinit:
# autoload -U compinit && compinit
```

Example for fish:
```sh
cfspeedtest --generate-completion fish > ~/.config/fish/completions/cfspeedtest.fish
```


## Development

### Logging
Set the log level using the `RUST_LOG` env var:  
```sh
RUST_LOG=debug cargo run
```
### Release
#### Using `cargo-release`
Install `cargo-release`:
```sh
cargo install cargo-release
```
Create the release (version bump levels are `[patch, minor, major]`):
```sh
cargo release patch --execute
```
This will bump the `cfspeedtest` version in both `Cargo.toml` and `Cargo.lock` and run `cargo publish` to push the release on crates.io. Additionally a version git tag is created and pushed to `master` triggering the GH action that creates the binary releases.

#### On GitHub
Release builds are published automatically using github actions. They are triggered when a git tag in the format `v[0-9]+.*` is pushed.
```sh
git tag v1.0.0
git push origin v1.0.0
```
#### On crates.io
1. Update `cfspeedtest` version in `Cargo.toml`
2. `cargo publish --dry-run`
3. Verify contents using `cargo package --list`
4. Upload to crates.io `cargo publish`
