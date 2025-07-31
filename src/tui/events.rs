use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Quit,
    Tick,
    Key(KeyCode),
}

pub struct EventHandler;

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler {
    pub fn new() -> Self {
        Self
    }

    pub fn next_event(&self, timeout: Duration) -> io::Result<Option<AppEvent>> {
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => Ok(Some(AppEvent::Quit)),
                    code => Ok(Some(AppEvent::Key(code))),
                },
                _ => Ok(None),
            }
        } else {
            Ok(Some(AppEvent::Tick))
        }
    }
}
