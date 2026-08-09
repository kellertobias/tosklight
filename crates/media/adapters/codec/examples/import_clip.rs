//! The import path, end to end, against a real clip.
//!
//! FFmpeg decodes out of process into raw RGBA; this repository compresses to HAP Alpha. That is
//! the whole conversion contract, so this example is how it gets measured on real footage rather
//! than on a synthetic pattern that flatters the compressor.
//!
//! `cargo run --release -p media-codec --example import_clip -- <file> [max-frames]`

use std::io::Read as _;
use std::process::{Command, Stdio};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(path) = arguments.next() else {
        eprintln!("usage: import_clip <file> [max-frames]");
        std::process::exit(2);
    };
    let limit: usize = arguments
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(240);

    let (width, height) = probe(&path);
    println!("{path}: {width}x{height}");

    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-v", "error", "-i", &path, "-f", "rawvideo", "-pix_fmt", "rgba", "-",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("ffmpeg is on PATH; import requires it");
    let mut output = ffmpeg.stdout.take().expect("piped");

    let frame_bytes = width as usize * height as usize * 4;
    let mut raw = vec![0u8; frame_bytes];
    let mut frames = 0usize;
    let mut compressed = 0usize;
    let started = std::time::Instant::now();

    while frames < limit {
        match output.read_exact(&mut raw) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => panic!("reading decoded frames: {error}"),
        }
        compressed += media_codec::encode(width, height, &raw)
            .expect("encodes")
            .len();
        frames += 1;
    }
    let elapsed = started.elapsed();
    let _ = ffmpeg.kill();
    let _ = ffmpeg.wait();

    if frames == 0 {
        println!("  no frames decoded");
        return;
    }

    let per_frame = compressed as f64 / frames as f64;
    let fps = frames as f64 / elapsed.as_secs_f64();
    println!(
        "  {frames} frames in {:.2}s — {fps:.0} fps end to end",
        elapsed.as_secs_f64()
    );
    println!(
        "  {:.2} MB per frame, {:.0} MB/s at 60 fps, {:.0} MB/s at 30 fps",
        per_frame / 1e6,
        per_frame * 60.0 / 1e6,
        per_frame * 30.0 / 1e6
    );
    println!(
        "  {:.0}% of uncompressed BC3, {:.1}% of raw RGBA",
        per_frame / media_codec::block_bytes(width, height) as f64 * 100.0,
        per_frame / frame_bytes as f64 * 100.0
    );
}

fn probe(path: &str) -> (u32, u32) {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
            path,
        ])
        .output()
        .expect("ffprobe is on PATH");
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.trim().split('x');
    let width = parts
        .next()
        .and_then(|v| v.parse().ok())
        .expect("a video width");
    let height = parts
        .next()
        .and_then(|v| v.parse().ok())
        .expect("a video height");
    (width, height)
}
