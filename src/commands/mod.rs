use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Context;
use crate::ui;

pub fn save_selection(
    selection: &ui::Selection,
    all_captures: &[ui::CapturedMonitor],
) -> anyhow::Result<String> {
    let captured = all_captures
        .iter()
        .find(|c| c.key == selection.monitor)
        .context("Selection monitor not found in captures")?;

    let cropped = image::imageops::crop_imm(
        &captured.image,
        selection.x,
        selection.y,
        selection.width,
        selection.height,
    )
    .to_image();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let downloads = dirs::download_dir().expect("Failed to determine downloads directory");

    // Encode PNG bytes in memory
    let mut png_bytes = Vec::new();
    cropped
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .context("Failed to encode PNG bytes")?;

    // Save PNG (lossless)
    let png_filename = format!("shotmd-{}.png", timestamp);
    let png_path = downloads.join(&png_filename);
    std::fs::write(&png_path, &png_bytes)
        .with_context(|| format!("Failed to save PNG screenshot to {}", png_path.display()))?;


    // Save JPEG (compressed, quality 85)
    let jpg_filename = format!("shotmd-{}.jpg", timestamp);
    let jpg_path = downloads.join(&jpg_filename);
    {
        use image::ExtendedColorType;
        use image::codecs::jpeg::JpegEncoder;
        let rgb = image::DynamicImage::from(cropped).into_rgb8();
        let mut jpg_buf = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpg_buf, 85);
        encoder
            .encode(&rgb, rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .with_context(|| format!("Failed to encode JPEG to {}", jpg_path.display()))?;
        std::fs::write(&jpg_path, &jpg_buf)
            .with_context(|| format!("Failed to save JPEG screenshot to {}", jpg_path.display()))?;
    }

    // Copy HTML embed with base64 PNG to clipboard
    {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        let html = format!("<img src=\"data:image/png;base64,{}\" />", b64);
        let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;

        // On Linux (X11), clipboard ownership is tied to the client process.
        // Block in save_selection() via SetExtLinux::wait() until another X11
        // client claims the selection, keeping the clipboard data alive.
        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            clipboard
                .set()
                .wait()
                .text(&html)
                .context("Failed to write HTML embed to clipboard")?;
        }

        #[cfg(not(target_os = "linux"))]
        clipboard
            .set_text(&html)
            .context("Failed to write HTML embed to clipboard")?;
    }

    Ok(png_path.to_string_lossy().to_string())
}

pub fn record_selection(selection: &ui::Selection, duration_secs: u64) -> anyhow::Result<String> {
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::RecvTimeoutError;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use xcap::Monitor;

    let monitor = Monitor::from_point(selection.monitor.x, selection.monitor.y)
        .context("Failed to get monitor from point")?;

    let (video_recorder, rx) = monitor
        .video_recorder()
        .context("Failed to create video recorder")?;

    let frames = Arc::new(Mutex::new(Vec::new()));
    let frames_clone = frames.clone();

    let recording_done = Arc::new(AtomicBool::new(false));
    let recording_done_clone = recording_done.clone();

    let collector = thread::spawn(move || {
        let mut collected = frames_clone.lock().unwrap();
        loop {
            if recording_done_clone.load(Ordering::SeqCst) {
                // Drain any remaining frames already buffered in the channel
                while let Ok(frame) = rx.try_recv() {
                    collected.push(frame);
                }
                break;
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(frame) => collected.push(frame),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    thread::sleep(Duration::from_millis(500));
    video_recorder
        .start()
        .context("Failed to start recording")?;
    thread::sleep(Duration::from_secs(duration_secs));
    video_recorder.stop().context("Failed to stop recording")?;

    // Signal the collector to stop. xcap's internal background thread holds
    // a cloned sender even after stop(), so the channel never closes naturally.
    recording_done.store(true, Ordering::SeqCst);
    collector.join().expect("collector thread panicked");

    let frames = frames.lock().unwrap();
    let frame_count = frames.len();
    if frame_count == 0 {
        anyhow::bail!("No frames captured");
    }

    let delay_ms = ((duration_secs * 1000) as u32) / (frame_count as u32);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let downloads = dirs::download_dir().expect("Failed to determine downloads directory");
    let filename = format!("shotmd-{}.gif", timestamp);
    let save_path = downloads.join(&filename);

    let mut gif_buf = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut gif_buf);
        encoder
            .set_repeat(image::codecs::gif::Repeat::Infinite)
            .context("Failed to set repeat")?;

        for frame in frames.iter() {
            let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.raw.clone())
                .context("Failed to create RgbaImage from frame")?;
            let cropped = image::imageops::crop_imm(
                &rgba,
                selection.x,
                selection.y,
                selection.width,
                selection.height,
            )
            .to_image();
            let gif_frame = image::Frame::from_parts(
                cropped,
                0,
                0,
                image::Delay::from_numer_denom_ms(delay_ms, 1),
            );
            encoder
                .encode_frame(gif_frame)
                .context("Failed to encode GIF frame")?;
        }
    }

    // Save compressed GIF to file
    std::fs::write(&save_path, &gif_buf)
        .with_context(|| format!("Failed to save compressed GIF to {}", save_path.display()))?;


    // Build HTML embed with the GIF as base64 for clipboard
    let html = {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&gif_buf);
        format!(
            "<img src=\"data:image/gif;base64,{}\" />",
            b64
        )
    };

    // Copy HTML embed with base64 GIF to clipboard
    {
        let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;

        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            clipboard
                .set()
                .wait()
                .text(&html)
                .context("Failed to write GIF HTML embed to clipboard")?;
        }

        #[cfg(not(target_os = "linux"))]
        clipboard
            .set_text(&html)
            .context("Failed to write GIF HTML embed to clipboard")?;
    }

    Ok(save_path.to_string_lossy().to_string())
}