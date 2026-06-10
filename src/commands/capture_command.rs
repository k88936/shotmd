use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Context;
use crate::commands::clipboard_utils;
use crate::ui;

pub fn capture_selection(
    selection: &ui::Selection,
    all_captures: &[ui::CapturedMonitor],
) -> anyhow::Result<()> {
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

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    let html = format!("<img src=\"data:image/png;base64,{}\" />", b64);
    clipboard_utils::save_to_clipboard(&html)?;
    Ok(())
}