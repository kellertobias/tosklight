//! Import a real source into a `.toskclip` and read it back.
//!
//! `cargo run --release -p media-codec --example import_to_clip -- <source> <destination>`
fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(source), Some(destination)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: import_to_clip <source> <destination.toskclip>");
        std::process::exit(2);
    };
    let source = std::path::PathBuf::from(source);
    let destination = std::path::PathBuf::from(destination);

    let started = std::time::Instant::now();
    let mut last = 0u32;
    let frames = media_codec::import(&source, &destination, &mut |progress| {
        match progress {
            media_codec::Progress::Started {
                width,
                height,
                frames,
            } => {
                println!(
                    "  {width}x{height}, {} frames",
                    frames.map_or("unknown".into(), |f| f.to_string())
                );
            }
            media_codec::Progress::Encoded { frame, total } => {
                if frame - last >= 100 {
                    last = frame;
                    match progress.fraction() {
                        Some(done) => {
                            println!("  {frame}/{} ({:.0}%)", total.unwrap(), done * 100.0)
                        }
                        None => println!("  {frame} frames"),
                    }
                }
            }
            media_codec::Progress::Finished { frames, bytes } => {
                println!("  wrote {frames} frames, {:.1} MB", bytes as f64 / 1e6);
            }
        }
        true
    })
    .expect("import succeeds");
    let elapsed = started.elapsed();
    println!(
        "  {frames} frames in {:.1}s ({:.0} fps)",
        elapsed.as_secs_f64(),
        frames as f64 / elapsed.as_secs_f64()
    );

    // Read it back the way playback will.
    let mut reader = media_codec::ClipReader::open(std::fs::File::open(&destination).unwrap())
        .expect("the clip reads back");
    let timing = reader.timing();
    println!(
        "  reads back: {} frames, last frame {:?}, duration {:?}, tempo {:?}",
        reader.header().frame_count,
        timing.last_frame,
        timing.duration,
        timing.intrinsic_bpm
    );

    let load = std::time::Instant::now();
    let resident = reader.read_resident().expect("loads into memory");
    println!(
        "  resident: {:.1} MB in {:.2}s ({:.0} MB/s)",
        resident.bytes() as f64 / 1e6,
        load.elapsed().as_secs_f64(),
        resident.bytes() as f64 / 1e6 / load.elapsed().as_secs_f64()
    );
}
