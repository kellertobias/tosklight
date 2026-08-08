//! Prints what this machine's GPU can do, so a codec decision rests on measurement.
//!
//! `cargo run -p media-render --example capabilities`
fn main() {
    let gpu = media_render::Gpu::off_screen().expect("an adapter is available");
    let features = gpu.adapter.features();
    println!(
        "adapter: {} ({}){}",
        gpu.capabilities.adapter_name,
        gpu.capabilities.backend,
        if gpu.capabilities.is_software {
            " [software]"
        } else {
            ""
        }
    );
    for (name, feature) in [
        (
            "BC   (DXT/S3TC — what HAP stores)",
            wgpu::Features::TEXTURE_COMPRESSION_BC,
        ),
        ("ETC2", wgpu::Features::TEXTURE_COMPRESSION_ETC2),
        ("ASTC", wgpu::Features::TEXTURE_COMPRESSION_ASTC),
    ] {
        println!("  {name}: {}", features.contains(feature));
    }
}
