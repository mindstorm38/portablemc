//! Various utilities to ease outputting human or machine readable text.

use std::time::{Duration, Instant};

pub mod human;
pub mod machine;


/// A common download handler to compute various metrics.
#[derive(Debug)]
pub struct DownloadTracker {
    /// If a download is running, this contains the instant it started, for speed calc.
    download_start: Option<Instant>,
}

#[derive(Debug)]
pub struct DownloadMetrics {
    /// Elapsed time since download started.
    pub elapsed: Duration,
    /// Average speed since download started (bytes/s).
    pub speed: f32,
}

impl DownloadTracker {

    pub fn new() -> Self {
        Self { download_start: None }
    }

    /// Handle progress of a download, returning some metrics if computable.
    pub fn handle(&mut self, count: u32, total_count: u32, size: u32, total_size: u32) -> Option<DownloadMetrics> {

        let _ = total_size;
        
        if self.download_start.is_none() {
            self.download_start = Some(Instant::now());
        }

        if size == 0 {
            if count == total_count {
                // If all entries have been downloaded but the weight nothing, reset the
                // download start. This is possible with zero-sized files or cache mode.
                self.download_start = None;
            }
            return None;
        }

        let elapsed = self.download_start.unwrap().elapsed();
        let speed = size as f32 / elapsed.as_secs_f32();

        if count == total_count {
            self.download_start = None;
        }

        Some(DownloadMetrics {
            elapsed,
            speed,
        })

    }

}
