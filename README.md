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
      --ipv4
          Force usage of IPv4
      --ipv6
          Force usage of IPv6
  -d, --disable-dynamic-max-payload-size
          Disables dynamically skipping tests with larger payload sizes if the tests for the previous payload size took longer than 5 seconds
      --download-only
          Test download speed only
      --upload-only
          Test upload speed only
      --generate-completion <COMPLETION>
          Generate shell completion script for the specified shell [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help
          Print help
  -V, --version
          Print version
```

Example usage:  
[![asciicast](https://asciinema.org/a/Moun5mFB1sm1VFkkFljG9UGyz.svg)](https://asciinema.org/a/Moun5mFB1sm1VFkkFljG9UGyz)

Example with json-pretty output:  
[![asciicast](https://asciinema.org/a/P6IUAADtaCq3bT18GbYVHmksA.svg)](https://asciinema.org/a/P6IUAADtaCq3bT18GbYVHmksA)

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
