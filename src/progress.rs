use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub struct ProgressIndicator {
    bar: ProgressBar,
}

impl ProgressIndicator {
    pub fn new_spinner(message: &str) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
        );
        bar.set_message(message.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));

        Self { bar }
    }

    pub fn set_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    pub fn finish_and_clear(&self) {
        self.bar.finish_and_clear();
    }
}

pub struct FileProcessingProgress {
    bar: ProgressBar,
}

impl FileProcessingProgress {
    pub fn new(total_files: usize) -> Self {
        let bar = ProgressBar::new(total_files as u64);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("📁 Processing files [{bar:25.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );

        Self { bar }
    }

    pub fn process_file(&self, filename: &str) {
        self.bar.set_message(format!("Processing {}", filename));
        self.bar.inc(1);
    }

    pub fn finish(&self) {
        self.bar.finish_with_message("✅ All files processed");
    }
}

pub struct ApiProgress {
    bar: ProgressBar,
}

impl ApiProgress {
    pub fn new_upload() -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("🚀 {msg} {spinner:.green}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
        );
        bar.set_message("Uploading verification request...");
        bar.enable_steady_tick(Duration::from_millis(80));

        Self { bar }
    }

    pub fn new_polling() -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("⏳ {msg} {spinner:.yellow}")
                .unwrap()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
        );
        bar.set_message("Checking verification status...");
        bar.enable_steady_tick(Duration::from_millis(120));

        Self { bar }
    }

    pub fn set_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }

    pub fn finish_with_message(&self, message: &str) {
        self.bar.finish_with_message(message.to_string());
    }

    pub fn finish_and_clear(&self) {
        self.bar.finish_and_clear();
    }
}
