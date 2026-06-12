use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};
use xcap::{Frame, Monitor};
use anyhow::Context;

pub struct RecordIter {
    rx: Receiver<Frame>,
    recording_done: Arc<AtomicBool>,
    timer: Option<thread::JoinHandle<anyhow::Result<()>>>,
    start_time: Option<Instant>,
    last_frame_time: Option<Instant>,
}

pub struct RecordFrame {
    pub frame: Frame,
    pub delay_ms: u32,
    pub timestamp_ms: u32,
}
impl Iterator for RecordIter {
    type Item = RecordFrame;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.recording_done.load(Ordering::SeqCst) {
                // Drain any remaining frames already buffered in the channel
                return match self.rx.try_recv() {
                    Ok(frame) => {
                        let now = Instant::now();
                        let start = *self.start_time.get_or_insert(now);
                        let timestamp_ms = now.duration_since(start).as_millis() as u32;
                        let delay_ms = match self.last_frame_time {
                            Some(last) => now.duration_since(last).as_millis() as u32,
                            None => 0,
                        };
                        self.last_frame_time = Some(now);
                        Some(RecordFrame { frame, delay_ms, timestamp_ms })
                    }
                    Err(_) => None,
                };
            }
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(frame) => {
                    let now = Instant::now();
                    let start = *self.start_time.get_or_insert(now);
                    let timestamp_ms = now.duration_since(start).as_millis() as u32;
                    let delay_ms = match self.last_frame_time {
                        Some(last) => now.duration_since(last).as_millis() as u32,
                        None => 0,
                    };
                    self.last_frame_time = Some(now);
                    return Some(RecordFrame { frame, delay_ms, timestamp_ms });
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

impl Drop for RecordIter {
    fn drop(&mut self) {
        // Ensure clean shutdown if the iterator is dropped before recording finishes
        self.recording_done.store(true, Ordering::SeqCst);
        if let Some(handle) = self.timer.take() {
            let _ = handle.join();
        }
    }
}

pub fn record(
    monitor: &Monitor,
    duration_secs: u64,
) -> anyhow::Result<RecordIter> {
    let (video_recorder, rx) = monitor
        .video_recorder()
        .context("Failed to create video recorder")?;

    let recording_done = Arc::new(AtomicBool::new(false));
    let recording_done_clone = recording_done.clone();

    let timer = thread::spawn(move || -> anyhow::Result<()> {
        // Wait a bit for the window to close
        thread::sleep(Duration::from_millis(500));
        video_recorder
            .start()
            .context("Failed to start recording")?;
        thread::sleep(Duration::from_secs(duration_secs));
        video_recorder.stop().context("Failed to stop recording")?;
        // Signal the encoder to stop. xcap's internal background thread holds
        // a cloned sender even after stop(), so the channel never closes naturally.
        recording_done_clone.store(true, Ordering::SeqCst);
        Ok(())
    });

    Ok(RecordIter {
        rx,
        recording_done,
        timer: Some(timer),
        start_time: None,
        last_frame_time: None,
    })
}