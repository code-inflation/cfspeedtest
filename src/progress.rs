use std::io::stdout;
use std::io::Write;

pub fn print_progress(name: &str, curr: u32, max: u32) {
    const BAR_LEN: u32 = 30;
    let progress_line = ((curr as f32 / max as f32) * BAR_LEN as f32) as u32;
    let remaining_line = BAR_LEN - progress_line;
    print!(
        "\r{} [{}{}]",
        name,
        (0..progress_line).map(|_| "=").collect::<String>(),
        (0..remaining_line).map(|_| "-").collect::<String>(),
    );
    stdout().flush().expect("error printing progress bar");
}
