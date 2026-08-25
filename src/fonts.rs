static GOST_B_BYTES: &[u8] = include_bytes!("../assets/fonts/gosttypeb.ttf");

/// Write the bundled GOST type B font to the user data dir so fontconfig can find it.
/// Returns the font family name to use in CSS.
pub fn ensure_gost_font() -> &'static str {
    let font_dir = crate::config::zerkalo_data_dir().join("fonts");
    let font_path = font_dir.join("gosttypeb.ttf");
    if !font_path.exists() {
        let _ = std::fs::create_dir_all(&font_dir);
        let _ = std::fs::write(&font_path, GOST_B_BYTES);
        // Refresh fontconfig cache for user fonts
        std::process::Command::new("fc-cache")
            .arg("-f")
            .arg(font_dir.to_string_lossy().as_ref())
            .spawn()
            .ok();
    }
    "GOST type B"
}
