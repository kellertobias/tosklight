fn main() {
    // The frontend is read by `generate_context!()` while this crate compiles, and cargo has no
    // other reason to know it changed: without these, an edited interface leaves the previous one
    // embedded in a binary cargo reports as fresh.
    println!("cargo:rerun-if-changed=../../../.artifacts/build/frontend/viz-editor");
    println!("cargo:rerun-if-changed=../../../.artifacts/build/frontend/viz-editor/index.html");
    tauri_build::build()
}
