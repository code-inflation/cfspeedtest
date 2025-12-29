use indicatif::{ProgressBar, ProgressStyle};

pub struct Progress {
    bar: ProgressBar,
}

impl Progress {
    pub fn new(name: &str, max: u32) -> Self {
        let bar = ProgressBar::new(max as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{prefix:<15} [{bar:30}]")
                .unwrap()
                .progress_chars("=-"),
        );
        bar.set_prefix(name.to_string());
        Progress { bar }
    }

    pub fn set_position(&self, curr: u32) {
        self.bar.set_position(curr as u64);
    }

    pub fn finish(&self) {
        self.bar.finish();
    }
}
