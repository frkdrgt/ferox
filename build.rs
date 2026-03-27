fn main() {
    // Rebuild when icon source changes.
    println!("cargo:rerun-if-changed=assets/logo.png");

    // Generate assets/icon.ico from assets/logo.png (multi-size).
    generate_ico();

    // Embed Windows manifest + ICO into the EXE.
    // MSVC: full manifest + icon.
    // GNU (mingw/windres): icon only — windres cannot preprocess XML manifests.
    #[cfg(target_os = "windows")]
    {
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        let mut res = winresource::WindowsResource::new();
        if target_env == "msvc" {
            res.set_manifest_file("pgclient.exe.manifest");
        }
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            // Non-fatal — app still runs, just without embedded EXE icon.
            println!("cargo:warning=Could not embed Windows icon: {e}");
        }
    }
}

fn generate_ico() {
    let src = "assets/logo.png";
    let dst = "assets/icon.ico";

    let img = image::open(src).expect("assets/logo.png not found");

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for size in [256u32, 128, 64, 48, 32, 16] {
        let resized =
            img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.into_rgba8();
        let (w, h) = rgba.dimensions();
        let icon_img = ico::IconImage::from_rgba_data(w, h, rgba.into_raw());
        icon_dir.add_entry(
            ico::IconDirEntry::encode(&icon_img).expect("Failed to encode ICO frame"),
        );
    }

    let mut file = std::fs::File::create(dst).expect("Failed to create assets/icon.ico");
    icon_dir.write(&mut file).expect("Failed to write ICO");
}
