fn main() {
    let frontend = std::env::var("LIGHT_CONTROL_FRONTEND_DIR").unwrap_or_else(|_| {
        format!(
            "{}/../../.artifacts/build/frontend/control-ui",
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")
        )
    });
    println!("cargo:rustc-env=LIGHT_CONTROL_FRONTEND_DIR={frontend}");
    println!("cargo:rerun-if-env-changed=LIGHT_CONTROL_FRONTEND_DIR");
    println!("cargo:rerun-if-changed={frontend}");
    println!("cargo:rerun-if-changed={frontend}/index.html");
}
