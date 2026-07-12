#![forbid(unsafe_code)]

use light_output::{DmxFrame, artdmx_packet, sacn_data_packet};
use std::{
    env,
    hint::black_box,
    time::{Duration, Instant},
};

fn main() {
    let mut universes = 64_u16;
    let mut seconds = 5_u64;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--universes" => {
                universes = args
                    .next()
                    .expect("--universes requires a value")
                    .parse()
                    .expect("invalid universe count")
            }
            "--seconds" => {
                seconds = args
                    .next()
                    .expect("--seconds requires a value")
                    .parse()
                    .expect("invalid duration")
            }
            "--help" => {
                println!("light-benchmark [--universes N] [--seconds N]");
                return;
            }
            _ => panic!("unknown argument: {argument}"),
        }
    }
    let frame: DmxFrame = std::array::from_fn(|slot| (slot % 256) as u8);
    let cid = [0x42; 16];
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let started = Instant::now();
    let mut rendered_frames = 0_u64;
    let mut bytes = 0_u64;
    while Instant::now() < deadline {
        for universe in 1..=universes {
            bytes += black_box(artdmx_packet(universe, rendered_frames as u8, &frame)).len() as u64;
            bytes += black_box(sacn_data_packet(
                universe,
                rendered_frames as u8,
                &frame,
                cid,
                "Light benchmark",
                100,
                false,
            ))
            .len() as u64;
        }
        rendered_frames += 1;
    }
    let elapsed = started.elapsed().as_secs_f64();
    let fps = rendered_frames as f64 / elapsed;
    let target = 44.0;
    println!(
        "universes={universes} frames={rendered_frames} elapsed_seconds={elapsed:.3} frames_per_second={fps:.1} encoded_megabytes={:.1} target_hz={target} result={}",
        bytes as f64 / 1_000_000.0,
        if fps >= target { "PASS" } else { "FAIL" }
    );
    if fps < target {
        std::process::exit(1);
    }
}
