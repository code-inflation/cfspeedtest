use std::{
    fmt,
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

static WRITE_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) fn print(args: fmt::Arguments<'_>) {
    write(|stdout| stdout.write_fmt(args));
}

pub(crate) fn print_line(args: fmt::Arguments<'_>) {
    write(|stdout| writeln!(stdout, "{args}"));
}

/// Whether this process encountered an unexpected stdout write error.
pub fn write_failed() -> bool {
    WRITE_FAILED.load(Ordering::Relaxed)
}

/// Keep output failures separate from the outcome of the measurements.
pub fn write(output: impl FnOnce(&mut io::StdoutLock<'_>) -> io::Result<()>) {
    let result = {
        let mut stdout = io::stdout().lock();
        output(&mut stdout).and_then(|()| stdout.flush())
    };
    let Err(error) = result else { return };
    if error.kind() != io::ErrorKind::BrokenPipe && !WRITE_FAILED.swap(true, Ordering::Relaxed) {
        let _ = writeln!(io::stderr().lock(), "Failed to write to stdout: {error}");
    }
}
