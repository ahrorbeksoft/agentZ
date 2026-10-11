fn main() {
    // Release builds embed the assets with rust-embed's `include_bytes!`, which tracks the files
    // it found but not new ones, so an incremental build kept embedding an older set (an icon
    // added later drew nothing). Watching the folder rebuilds this crate when a file is added.
    println!("cargo:rerun-if-changed=../../assets");
}
