pub mod app;
pub mod theme;
pub mod ui;
pub mod widgets;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::engine::runner::run_speed_test;
use crate::engine::types::{SpeedTestConfig, SpeedTestEvent};

use app::App;

/// Run the full-screen TUI speed test.
pub async fn run(client: reqwest::Client, config: SpeedTestConfig) -> Result<()> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_inner(&mut terminal, client, config).await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_inner(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: reqwest::Client,
    config: SpeedTestConfig,
) -> Result<()> {
    let mut app = App::new(config.nr_latency_tests);

    let (tx, mut rx) = mpsc::channel::<SpeedTestEvent>(256);

    // Spawn engine
    let engine_client = client.clone();
    let engine_config = config.clone();
    let engine_handle =
        tokio::spawn(async move { run_speed_test(&engine_client, &engine_config, tx).await });

    let tick_rate = Duration::from_millis(50);

    loop {
        // Draw
        terminal.draw(|f| ui::draw(f, &app))?;

        // Drain all pending engine events
        loop {
            match rx.try_recv() {
                Ok(event) => app.handle_event(event),
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        // Check for key events (non-blocking)
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Wait for engine to finish (or abort)
    drop(rx);
    let _ = engine_handle.await;

    Ok(())
}
