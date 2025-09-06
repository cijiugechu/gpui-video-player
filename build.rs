fn main() {
    if cfg!(target_os = "macos") {
        // Add the GStreamer framework directory to the library search path
        println!("cargo:rustc-link-search=framework=/Library/Frameworks");

        // Add an rpath to the GStreamer framework directory
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,/Library/Frameworks/GStreamer.framework/Versions/1.0/lib"
        );
    }
}
