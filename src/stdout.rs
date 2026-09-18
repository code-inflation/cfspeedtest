//! Helpers for writing terminal output to stdout without panicking.
//!
//! This CLI's reports are routinely piped into other tools, e.g.
//! `cfspeedtest -o json | jq`. When such a consumer exits before the run
//! finishes, stdout writes fail with [`io::ErrorKind::BrokenPipe`]. The
//! `print!`/`println!` macros panic on any write error, so all stdout output
//! goes through these helpers instead: a broken pipe is tolerated silently
//! because the consumer is simply gone, while unexpected write errors are
//! reported once on stderr (the diagnostic channel) without aborting the run.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

static REPORTED_WRITE_ERROR: AtomicBool = AtomicBool::new(false);

/// Writes `text` to stdout (without trailing newline) and flushes.
pub fn print(text: &str) {
    let mut stdout = io::stdout().lock();
    if let Err(error) = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        handle_write_error(&error);
    }
}

/// Writes `text` followed by a newline to stdout and flushes.
pub fn print_line(text: &str) {
    print(&format!("{text}\n"));
}

/// Reports a failed stdout write. Broken pipes are ignored; any other error
/// is reported on stderr, at most once per process.
pub fn handle_write_error(error: &io::Error) {
    handle_write_kind(error.kind(), &format!("Failed to write to stdout: {error}"));
}

/// Reports a failed stdout write by error kind. Broken pipes are ignored;
/// any other error is reported on stderr, at most once per process.
pub fn handle_write_kind(kind: io::ErrorKind, message: &str) {
    if kind == io::ErrorKind::BrokenPipe {
        // The consumer of our stdout went away; keep going without output.
        return;
    }
    if !REPORTED_WRITE_ERROR.swap(true, Ordering::Relaxed) {
        eprintln!("{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broken_pipe_is_tolerated() {
        // The handler must classify a broken pipe as tolerable instead of
        // panicking; there is no stdout to assert on.
        handle_write_kind(io::ErrorKind::BrokenPipe, "unreachable");
    }
}
