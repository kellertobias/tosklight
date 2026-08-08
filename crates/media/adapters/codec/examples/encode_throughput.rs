//! How fast import can normalise to HAP Alpha on this machine.
//!
//! `cargo run --release -p media-codec --example encode_throughput`
fn main() {
    let (width, height) = (1920u32, 1080u32);
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    // Something with real detail: a flat fill would flatter the compressor.
    for (index, pixel) in rgba.chunks_exact_mut(4).enumerate() {
        let x = (index as u32 % width) as u8;
        let y = (index as u32 / width) as u8;
        pixel.copy_from_slice(&[x, y, x ^ y, 255 - (x / 2)]);
    }

    let frames = 30;
    let started = std::time::Instant::now();
    let mut bytes = 0usize;
    for _ in 0..frames {
        bytes += media_codec::encode(width, height, &rgba)
            .expect("encodes")
            .len();
    }
    let elapsed = started.elapsed();

    let fps = frames as f64 / elapsed.as_secs_f64();
    println!(
        "1080p HAP Alpha encode: {fps:.1} fps ({:.1}x realtime at 60 fps)",
        fps / 60.0
    );
    println!(
        "  {:.2} MB per frame, {:.0} MB/s at 60 fps playback \
         (synthetic content; real footage compresses less well)",
        bytes as f64 / frames as f64 / 1e6,
        bytes as f64 / frames as f64 * 60.0 / 1e6
    );
}
