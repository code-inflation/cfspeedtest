use std::{
    fmt,
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

static REPORTED_WRITE_ERROR: AtomicBool = AtomicBool::new(false);

pub(crate) fn print(args: fmt::Arguments<'_>) {
    write(|stdout| stdout.write_fmt(args));
}

pub(crate) fn print_line(args: fmt::Arguments<'_>) {
    write(|stdout| writeln!(stdout, "{args}"));
}

/// Keep output failures separate from the outcome of the measurements.
pub fn write(output: impl FnOnce(&mut io::StdoutLock<'_>) -> io::Result<()>) {
    let result = {
        let mut stdout = io::stdout().lock();
        output(&mut stdout).and_then(|()| stdout.flush())
    };
    let Err(error) = result else { return };
    if error.kind() != io::ErrorKind::BrokenPipe
        && !REPORTED_WRITE_ERROR.swap(true, Ordering::Relaxed)
    {
        let _ = writeln!(io::stderr().lock(), "Failed to write to stdout: {error}");
    }
}
