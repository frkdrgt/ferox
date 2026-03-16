fn main() {
    // Embed Windows manifest and icon only when building with MSVC toolchain.
    // GNU toolchain (MSYS2/mingw) skips this — manifest is not needed for dev builds.
    #[cfg(target_os = "windows")]
    {
        let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        if target_env == "msvc" {
            let mut res = winresource::WindowsResource::new();
            res.set_manifest_file("pgclient.exe.manifest");
            // Uncomment when assets/icon.ico is ready:
            // res.set_icon("assets/icon.ico");
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
