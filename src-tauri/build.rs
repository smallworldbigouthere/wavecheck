fn main() {
    // Expose the build target triple so the engine can locate the correct
    // sidecar binary (e.g. ffmpeg-aarch64-apple-darwin) during `tauri dev`.
    println!(
        "cargo:rustc-env=TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap_or_default()
    );
    tauri_build::build()
}
