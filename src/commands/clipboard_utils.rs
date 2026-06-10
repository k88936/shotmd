use anyhow::Context;

pub fn save_to_clipboard(html: &String) -> anyhow::Result<()> {
    // Copy HTML embed with base64 WebP to clipboard
    #[cfg(not(any(test)))]
    {
        let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;

        #[cfg(target_os = "linux")]
        {
            use arboard::SetExtLinux;
            clipboard
                .set()
                .wait()
                .text(html)
                .context("Failed to write WebP HTML embed to clipboard")?;
        }

        #[cfg(not(target_os = "linux"))]
        clipboard
            .set_text(&html)
            .context("Failed to write WebP HTML embed to clipboard")?;
    }
    Ok(())
}
