use crate::commands::{clipboard_utils, record_utils};
use webp_animation::prelude::*;
use crate::ui;
use crate::ui::Selection;
use anyhow::Context;
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;


pub fn record_selection(selection: &ui::Selection, duration_secs: u64) -> anyhow::Result<()> {
    let webp_buf = record_to_webp(selection, duration_secs)?;

    if webp_buf.is_empty() {
        anyhow::bail!("No frames captured");
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let downloads = dirs::download_dir().expect("Failed to determine downloads directory");
    let filename = format!("shotmd-{}.webp", timestamp);
    let save_path = downloads.join(&filename);

    // Save animated WebP to file (already encoded streamingly)
    std::fs::write(&save_path, &webp_buf)
        .with_context(|| format!("Failed to save WebP to {}", save_path.display()))?;

    // Build HTML embed with the WebP as base64 for clipboard
    let html = {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&webp_buf);
        format!("<img src=\"data:image/webp;base64,{}\" />", b64)
    };

    clipboard_utils::save_to_clipboard(&html)?;

    Ok(())
}

pub fn record_to_webp(selection: &Selection, duration_secs: u64) -> anyhow::Result<Vec<u8>> {
    let monitor = Monitor::from_point(selection.monitor.x, selection.monitor.y)
        .context("Failed to get monitor from point")?;

    let sel_x = selection.x;
    let sel_y = selection.y;
    let sel_w = selection.width;
    let sel_h = selection.height;

    // Collect frames: (rgba_data, timestamp_ms)
    let rgba_frames: Vec<(Vec<u8>, u32)> = record_utils::record(&monitor, duration_secs)?
        .map(|rf| {
            let rgba = image::RgbaImage::from_raw(rf.frame.width, rf.frame.height, rf.frame.raw)
                .expect("Failed to convert frame to RGBA");
            let cropped = image::imageops::crop_imm(&rgba, sel_x, sel_y, sel_w, sel_h).to_image();
            (cropped.into_raw(), rf.timestamp_ms)
        })
        .collect();

    if rgba_frames.is_empty() {
        anyhow::bail!("No frames captured");
    }


    let mut encoder = Encoder::new((sel_w as u32, sel_h as u32))
        .context("Failed to create WebP animation encoder")?;

    for (rgba_data, ts) in &rgba_frames {
        encoder
            .add_frame(rgba_data, *ts as i32)
            .context("Failed to add frame to WebP encoder")?;
    }

    let final_timestamp = rgba_frames.last().map(|(_, ts)| *ts as i32).unwrap_or(0);
    let webp_data = encoder
        .finalize(final_timestamp)
        .context("Failed to finalize WebP animation")?;

    Ok(webp_data.to_vec())
}

