// Tells the asset embedder where the built React application is.
//
// The path is the canonical artifact location, and an explicit `LIGHT_MEDIA_FRONTEND_DIR`
// overrides it, so a packaging job can point at a frontend it built elsewhere.
fn main() {
    let frontend = std::env::var("LIGHT_MEDIA_FRONTEND_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../../../.artifacts/build/frontend/media",
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")
        )
    });
    // The embedder refuses to compile against a folder that is not there, and `cargo test` on a
    // clean checkout has no reason to have built a frontend yet. An empty directory embeds
    // nothing and the server says so at runtime, which is the useful failure.
    let _ = std::fs::create_dir_all(&frontend);
    println!("cargo:rustc-env=LIGHT_MEDIA_FRONTEND_DIR={frontend}");
    println!("cargo:rerun-if-env-changed=LIGHT_MEDIA_FRONTEND_DIR");
    println!("cargo:rerun-if-changed={frontend}");
    println!("cargo:rerun-if-changed={frontend}/index.html");
}
