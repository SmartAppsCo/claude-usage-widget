fn main() {
    #[cfg(target_os = "windows")]
    {
        // Create a minimal .ico (PNG-embedded) from our icon.png at build time,
        // then use winresource to compile it into the .exe as a Windows resource.
        // This gives the app a proper icon in the taskbar, file explorer, and ALT-TAB.
        let png = include_bytes!("images/icon.png");
        let ico = png_to_ico(png);
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let ico_path = std::path::Path::new(&out_dir).join("icon.ico");
        std::fs::write(&ico_path, &ico).unwrap();

        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.compile().unwrap();
    }
}

/// Wrap raw PNG bytes in a minimal ICO container (single 256x256 entry).
#[cfg(target_os = "windows")]
fn png_to_ico(png: &[u8]) -> Vec<u8> {
    let size = png.len() as u32;
    let mut ico = Vec::with_capacity(22 + png.len());
    // ICO header
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    ico.extend_from_slice(&1u16.to_le_bytes()); // count = 1
    // Directory entry
    ico.push(0); // width  (0 = 256)
    ico.push(0); // height (0 = 256)
    ico.push(0); // color count
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&size.to_le_bytes()); // image data size
    ico.extend_from_slice(&22u32.to_le_bytes()); // offset to image data
    // PNG data
    ico.extend_from_slice(png);
    ico
}
