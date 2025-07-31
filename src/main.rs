use cfspeedtest::speedtest;
use cfspeedtest::speedtest_tui;
use cfspeedtest::tui::App;
use cfspeedtest::OutputFormat;
use cfspeedtest::SpeedTestCLIOptions;
use clap::{CommandFactory, Parser};
use clap_complete::generate;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::net::IpAddr;
use std::thread;

use speedtest::speed_test;

fn print_completions<G: clap_complete::Generator>(gen: G, cmd: &mut clap::Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn run_tui_mode(client: reqwest::blocking::Client, options: SpeedTestCLIOptions) {
    // Setup terminal
    enable_raw_mode().expect("Failed to enable raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("Failed to enter alternate screen");
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).expect("Failed to create terminal");

    // Create channel for communication between speedtest and TUI
    let (event_sender, event_receiver) = crossbeam_channel::unbounded();

    // Create and configure the app
    let mut app = App::new().with_receiver(event_receiver);

    // Start speedtest in a separate thread
    let event_sender_clone = event_sender.clone();
    let client_clone = client.clone();
    let options_clone = options.clone();

    thread::spawn(move || {
        speedtest_tui::speed_test_tui(client_clone, options_clone, event_sender_clone);
    });

    // Run the TUI
    let result = app.run(&mut terminal);

    // Cleanup terminal
    disable_raw_mode().expect("Failed to disable raw mode");
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .expect("Failed to leave alternate screen");
    terminal.show_cursor().expect("Failed to show cursor");

    if let Err(err) = result {
        eprintln!("TUI error: {}", err);
    }
}

fn main() {
    env_logger::init();
    let options = SpeedTestCLIOptions::parse();

    if let Some(generator) = options.completion {
        let mut cmd = SpeedTestCLIOptions::command();
        eprintln!("Generating completion script for {generator}...");
        print_completions(generator, &mut cmd);
        return;
    }

    if options.output_format == OutputFormat::StdOut {
        println!("Starting Cloudflare speed test");
    }
    let client;
    if let Some(ref ip) = options.ipv4 {
        client = reqwest::blocking::Client::builder()
            .local_address(ip.parse::<IpAddr>().expect("Invalid IPv4 address"))
            .timeout(std::time::Duration::from_secs(30))
            .build();
    } else if let Some(ref ip) = options.ipv6 {
        client = reqwest::blocking::Client::builder()
            .local_address(ip.parse::<IpAddr>().expect("Invalid IPv6 address"))
            .timeout(std::time::Duration::from_secs(30))
            .build();
    } else {
        client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build();
    }
    let client = client.expect("Failed to initialize reqwest client");

    if options.tui {
        run_tui_mode(client, options);
    } else {
        speed_test(client, options);
    }
}
