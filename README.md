# cfspeedtest - Unofficial CLI for [speed.cloudflare.com](https://speed.cloudflare.com)
[![Build](https://github.com/code-inflation/cfspeedtest/actions/workflows/CI.yml/badge.svg?branch=master)](https://github.com/code-inflation/cfspeedtest/actions/workflow[![CI](https://github.com/code-inflation/cfspeedtest/actions/workflows/CI.yml/badge.svg)](https://github.com/code-inflation/cfspeedtest/actions/workflows/CI.yml)s/CI.yml)

## Installation
Install using `cargo`:
```sh
cargo install cfspeedtest
```

Or download the latest binary release here: [cfspeedtest/releases/latest](https://github.com/code-inflation/cfspeedtest/releases/latest)

## Usage
```
> cfspeedtest --help
Unofficial CLI for speed.cloudflare.com

Usage: cfspeedtest [OPTIONS]

Options:
  -n, --nr-tests <NR_TESTS>
          Number of test runs per payload size. Needs to be at least 4 [default: 10]
      --nr-latency-tests <NR_LATENCY_TESTS>
          Number of latency tests to run [default: 25]
  -m, --max-payload-size <MAX_PAYLOAD_SIZE>
          The max payload size in bytes to use [100k, 1m, 10m, 25m or 100m] [default: 10MB]
  -o, --outupt-format <OUTUPT_FORMAT>
          Set the output format [csv, json or json-pretty] > This silences all other output to stdout
  -v, --verbose
          Enable verbose output i.e. print out boxplots of the measurements
  -h, --help
          Print help
  -V, --version
          Print version
```

Example usage:  
[![asciicast](https://asciinema.org/a/AnIXZQ653VbcNtcAr6fFJWwj6.svg)](https://asciinema.org/a/AnIXZQ653VbcNtcAr6fFJWwj6)

Example with json-pretty output:  
[![asciicast](https://asciinema.org/a/xmktVNE8Ei5FYPqKBKEk658lt.svg)](https://asciinema.org/a/xmktVNE8Ei5FYPqKBKEk658lt)


## Development
### TODO
- [ ] Dynamic payload sizing depending on network speed
- [X] Consider server processing time in measurements
- [X] ~~CLI arguments (nr of tests, payload sizes, verbosity)~~
- [X] Clean up output
- [X] Boxplot for measurements
- [X] Asciinema recording in readme
- [X] Publish crate
- [X] ~~Install and~~ ~~usage instructions~~
- [X] ~~Add Serde to provide CSV/ JSON and JSON-pretty output~~

### Logging
Set the log level using the `RUST_LOG` env var:  
```sh
RUST_LOG=debug cargo run
```
### Release
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
