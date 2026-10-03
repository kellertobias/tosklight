//! TL-639 round 5: a frame's output is the same for every output worker count. The digest of a
//! typed stress run (Position and Color Dynamics on two thousand fixtures, parallel family
//! preparation, composition and render) and of its lifecycle (fades, FixAT, Freeze, Preload GO)
//! must equal the single-threaded digest exactly: every fixture, DMX universe, point, event and
//! work counter of every tick. Planned Position forests are verified against the general walk
//! throughout.
use super::run;
use crate::light_benchmark::arguments::{Arguments, ParseOutcome};

fn digest(extra: &[&str], workers: &str) -> serde_json::Value {
    let packages = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fixture-library");
    let mut arguments = vec![
        "--fixture-package-dir",
        packages,
        "--protocol",
        "both",
        "--transport",
        "encode-only",
        "--output-workers",
        workers,
    ];
    arguments.extend_from_slice(extra);
    let ParseOutcome::Run(arguments) =
        Arguments::parse(arguments.into_iter().map(String::from)).unwrap()
    else {
        panic!("benchmark arguments")
    };
    run(&arguments, arguments.semantic.digest_ticks.unwrap()).unwrap()
}

#[test]
fn every_output_worker_count_renders_the_single_threaded_digest() {
    light_dynamics::set_plan_verification(true);
    for extra in [
        &[
            "--headless-stress-fixtures",
            "2000",
            "--semantic",
            "--digest-ticks",
            "3",
        ][..],
        &[
            "--headless-stress-fixtures",
            "2000",
            "--semantic",
            "--digest-ticks",
            "12",
            "--digest-lifecycle",
        ][..],
    ] {
        let single = digest(extra, "1");
        for workers in ["2", "5"] {
            assert!(
                digest(extra, workers) == single,
                "{workers} output workers changed the digest of {extra:?}"
            );
        }
    }
}
