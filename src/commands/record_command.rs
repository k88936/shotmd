use crate::ui;
use anyhow::Context;
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;
use crate::commands::{clipboard_utils, record_utils};
use crate::ui::Selection;

/// Encode a frame to a simple lossless WebP and extract the raw VP8L bitstream.
fn encode_vp8l_frame(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    use image_webp::{ColorType, WebPEncoder};
    let mut buf = Vec::new();
    WebPEncoder::new(&mut buf)
        .encode(data, width, height, ColorType::Rgb8)
        .unwrap();
    // Simple lossless WebP layout:
    // [0..4]   "RIFF"
    // [4..8]   file size (little-endian u32)
    // [8..12]  "WEBP"
    // [12..16] "VP8L"
    // [16..20] VP8L chunk data size (little-endian u32)
    // [20..]   VP8L bitstream data
    let vp8l_data_size = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
    buf[20..20 + vp8l_data_size].to_vec()
}

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

    // Copy selection coordinates for use in the spawned encoder thread
    let sel_x = selection.x;
    let sel_y = selection.y;
    let sel_w = selection.width;
    let sel_h = selection.height;

    let vp8l_frames: Vec<(Vec<u8>, u32)> = record_utils::record(&monitor, duration_secs)?
        .map(|(frame, delay_ms)| {
            let rgba = image::RgbaImage::from_raw(frame.width, frame.height, frame.raw)
                .expect("Failed to convert frame to RGBA");
            let cropped = image::imageops::crop_imm(&rgba, sel_x, sel_y, sel_w, sel_h).to_image();
            let rgb8 = image::DynamicImage::from(cropped).into_rgb8();
            let vp8l = encode_vp8l_frame(&rgb8, rgb8.width(), rgb8.height());
            (vp8l, delay_ms)
        })
        .collect();

    // Assemble animated WebP container with per-frame timing
    let webp_buf = build_animated_webp(
        &vp8l_frames,
        sel_w as u32,
        sel_h as u32,
        0, // loop_count = 0 for infinite
    );
    Ok(webp_buf)
}

/// Build a complete animated WebP from frame VP8L bitstreams with per-frame delays.
fn build_animated_webp(
    frames: &[(Vec<u8>, u32)], // (vp8l_data, frame_duration_ms)
    width: u32,
    height: u32,
    loop_count: u16,
) -> Vec<u8> {
    let mut output = Vec::new();

    // Reserve space for RIFF header (will patch the file size later)
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&[0u8; 4]);
    output.extend_from_slice(b"WEBP");

    // --- VP8X chunk (10 bytes of data) ---
    let mut vp8x_data = Vec::with_capacity(10);
    vp8x_data.push(0b0000_0010); // flags: animation bit set
    vp8x_data.extend_from_slice(&[0u8; 3]); // reserved
    vp8x_data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // canvas width - 1
    vp8x_data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // canvas height - 1

    output.extend_from_slice(b"VP8X");
    output.extend_from_slice(&(vp8x_data.len() as u32).to_le_bytes());
    output.extend_from_slice(&vp8x_data);

    // --- ANIM chunk (6 bytes of data) ---
    let mut anim_data = Vec::with_capacity(6);
    anim_data.extend_from_slice(&[0u8; 4]); // background color (BGRA) – default black
    anim_data.extend_from_slice(&loop_count.to_le_bytes());

    output.extend_from_slice(b"ANIM");
    output.extend_from_slice(&(anim_data.len() as u32).to_le_bytes());
    output.extend_from_slice(&anim_data);

    // --- ANMF chunks ---
    for (vp8l_data, frame_duration_ms) in frames {
        // ANMF data (16 bytes of header before subchunks)
        let mut anmf_data = Vec::new();

        // Frame position and size (3 bytes each, little-endian)
        anmf_data.extend_from_slice(&[0u8; 3]); // frame X offset
        anmf_data.extend_from_slice(&[0u8; 3]); // frame Y offset
        anmf_data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // frame width - 1
        anmf_data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // frame height - 1
        anmf_data.extend_from_slice(&frame_duration_ms.to_le_bytes()[..3]); // frame duration (3 bytes)
        anmf_data.push(0); // flags: no dispose, no blend

        // VP8L sub-chunk inside the ANMF
        anmf_data.extend_from_slice(b"VP8L");
        anmf_data.extend_from_slice(&(vp8l_data.len() as u32).to_le_bytes());
        anmf_data.extend_from_slice(vp8l_data);
        // VP8L sub-chunk padding
        if vp8l_data.len() % 2 == 1 {
            anmf_data.push(0);
        }

        // ANMF chunk: header + data
        output.extend_from_slice(b"ANMF");
        output.extend_from_slice(&(anmf_data.len() as u32).to_le_bytes());
        output.extend_from_slice(&anmf_data);
        // ANMF chunk padding
        if anmf_data.len() % 2 == 1 {
            output.push(0);
        }
    }

    // Patch the RIFF file size
    let file_size = (output.len() as u32).wrapping_sub(8);
    output[4..8].copy_from_slice(&file_size.to_le_bytes());

    output
}
