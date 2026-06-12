use crate::commands::{clipboard_utils, record_utils};
use webp_animation::prelude::*;
use crate::ui;
use crate::ui::Selection;
use anyhow::Context;
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;


pub fn record_selection(selection: &ui::Selection, duration_secs: u64) -> anyhow::Result<()> {
    let (rgba_frames, width, height) = collect_frames(selection, duration_secs)?;


    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let downloads = dirs::download_dir().expect("Failed to determine downloads directory");

    let filename = format!("shotmd-{}.webp", timestamp);
    let save_path = downloads.join(&filename);

    let webp_buf = encode_frames(&rgba_frames, width, height, &EncodingConfig::default())?;
    std::fs::write(&save_path, &webp_buf)
        .with_context(|| format!("Failed to save WebP to {}", save_path.display()))?;

    let compressed_filename = format!("shotmd-{}-compressed.webp", timestamp);
    let compressed_path = downloads.join(&compressed_filename);
    let compressed_buf = encode_frames(&rgba_frames, width, height, &EncodingConfig::new_lossy(75.0))?;
    std::fs::write(&compressed_path, &compressed_buf)
        .with_context(|| format!("Failed to save compressed WebP to {}", compressed_path.display()))?;

    let html = {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&compressed_buf);
        format!("<img src=\"data:image/webp;base64,{}\" />", b64)
    };

    clipboard_utils::save_to_clipboard(&html)?;

    Ok(())
}

fn collect_frames(selection: &Selection, duration_secs: u64) -> anyhow::Result<(Vec<(Vec<u8>, u32)>, u32, u32)> {
    let monitor = Monitor::from_point(selection.monitor.x, selection.monitor.y)
        .context("Failed to get monitor from point")?;

    let sel_x = selection.x;
    let sel_y = selection.y;
    let sel_w = selection.width;
    let sel_h = selection.height;

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

    Ok((rgba_frames, sel_w as u32, sel_h as u32))
}

fn encode_frames(
    rgba_frames: &[(Vec<u8>, u32)],
    width: u32,
    height: u32,
    config: &EncodingConfig,
) -> anyhow::Result<Vec<u8>> {
    let mut encoder = Encoder::new((width, height))
        .context("Failed to create WebP animation encoder")?;
    encoder
        .set_default_encoding_config(config.clone())
        .context("Failed to set encoding config")?;

    for (rgba_data, ts) in rgba_frames {
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

