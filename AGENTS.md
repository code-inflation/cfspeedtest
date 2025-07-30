# AGENTS.md - Development Guide for cfspeedtest

## Build/Test Commands
- `cargo build` - Build the project
- `cargo test` - Run all tests
- `cargo test test_name` - Run a specific test
- `cargo fmt` - Format code (required before commits)
- `cargo clippy` - Run linter
- `cargo run` - Run the CLI tool
- `cargo run --example simple_speedtest` - Run example

## Code Style Guidelines
- Use `cargo fmt` for consistent formatting
- Follow Rust 2021 edition conventions
- Use snake_case for functions/variables, PascalCase for types/enums
- Prefer explicit types in public APIs
- Use `Result<T, String>` for error handling with descriptive messages
- Import std modules first, then external crates, then local modules
- Use `log` crate for logging, `env_logger` for initialization
- Prefer `reqwest::blocking::Client` for HTTP requests
- Use `clap` derive macros for CLI argument parsing
- Write comprehensive unit tests in `#[cfg(test)]` modules
- Use `serde` for serialization with `#[derive(Serialize)]`
- Constants should be SCREAMING_SNAKE_CASE
- Prefer `Duration` and `Instant` for time measurements