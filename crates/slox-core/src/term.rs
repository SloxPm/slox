use std::io::{self, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) struct Report {
    summary: String,
    details: Vec<String>,
}

impl Report {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    pub(crate) fn detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }
}

pub(crate) struct Progress {
    started_at: Instant,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Progress {
    pub(crate) fn start(message: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let message = message.to_string();
        let handle = thread::spawn(move || {
            let frames = ['o', '0', 'O'];
            let mut index = 0usize;

            while !thread_stop.load(Ordering::Relaxed) {
                eprint!("\r{} {}", frames[index % frames.len()], message);
                let _ = io::stderr().flush();
                index += 1;
                thread::sleep(Duration::from_millis(120));
            }
        });

        Self {
            started_at: Instant::now(),
            stop,
            handle: Some(handle),
        }
    }

    pub(crate) fn finish(mut self, report: Report) {
        self.stop();
        let elapsed = format_duration(self.started_at.elapsed());

        eprintln!("{} in {}", report.summary, elapsed);
        for detail in report.details {
            eprintln!("- {}", detail);
        }
    }

    pub(crate) fn fail(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        eprint!("\r\x1b[2K");
        let _ = io::stderr().flush();
    }
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.1}s", duration.as_secs_f64())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

pub fn report_error(error: &str) {
    eprintln!("error {}", error);
}
