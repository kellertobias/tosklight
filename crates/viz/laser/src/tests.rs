use super::*;

fn request<'a>(source: &'a str, key: u64, slots: &'a [u8]) -> ScanRequest<'a> {
    ScanRequest {
        source,
        source_key: key,
        slots,
        time_seconds: 0.0,
        elapsed_seconds: 0.016,
        intensity: 1.0,
    }
}

fn engine() -> ScanEngine {
    ScanEngine::new().expect("a scan runtime must start")
}

/// The contract in one test: a module exporting `scan` gets the fixture's DMX and its output
/// becomes the path the beam takes, with the percentages turned into fractions of the scan.
#[test]
fn a_module_exporting_scan_produces_the_path_the_beam_takes() {
    const SOURCE: &str = r#"
        export function scan(input) {
          const level = input.dmx[0] / 255;
          return {
            points: [
              { x: -1, y: 0, r: level, g: 0, b: 0, amount: 25 },
              { x:  1, y: 0, r: level, g: 0, b: 0, amount: 75 },
            ],
            pointsPerSecond: 24000,
          };
        }
    "#;
    let scan = engine().scan(0, &request(SOURCE, 1, &[255, 0, 0]));
    assert_eq!(scan.fault, None);
    assert_eq!(scan.points.len(), 2);
    assert_eq!(scan.points[0].x, -1.0);
    assert_eq!(scan.points[0].colour, [1.0, 0.0, 0.0]);
    assert_eq!(scan.points[0].dwell, 0.25);
    assert_eq!(scan.points[1].dwell, 0.75);
    assert_eq!(scan.points_per_second, 24_000.0);
}

/// Percentages that do not add up are the normal case, not an error. A script emitting `100/n`
/// per point rounds, and a generated figure may not have thought about timing at all.
#[test]
fn dwell_shares_are_rescaled_to_one_whatever_the_script_emitted() {
    const LOPSIDED: &str = "export function scan() { return { points: [\
        { x: 0, y: 0, amount: 10 }, { x: 1, y: 1, amount: 10 }] }; }";
    let scan = engine().scan(0, &request(LOPSIDED, 1, &[]));
    let total: f32 = scan.points.iter().map(|point| point.dwell).sum();
    assert!((total - 1.0).abs() < 1e-6, "shares summed to {total}");

    const SILENT: &str = "export function scan() { return { points: [\
        { x: 0, y: 0 }, { x: 1, y: 0 }, { x: 1, y: 1 }, { x: 0, y: 1 }] }; }";
    let scan = engine().scan(0, &request(SILENT, 1, &[]));
    assert!(
        scan.points.iter().all(|point| point.dwell == 0.25),
        "a script that named no timing means an even sweep"
    );
}

/// A laser that has stopped projecting must say why. Every one of these is a fault an operator
/// can act on, and none of them may reach the renderer as an empty picture with no explanation.
#[test]
fn every_way_a_script_can_fail_is_reported_rather_than_silent() {
    let cases: [(&str, &str); 5] = [
        ("export function scan( {", "could not be compiled"),
        ("export const scan = 4;", "is not a function"),
        (
            "export function other() {}",
            "does not export a `scan` function",
        ),
        (
            "export function scan() { throw new Error('no pattern'); }",
            "no pattern",
        ),
        (
            "export function scan() { return { points: 7 }; }",
            "not an array",
        ),
    ];
    for (index, (source, expected)) in cases.iter().enumerate() {
        let scan = engine().scan(0, &request(source, index as u64, &[]));
        let fault = scan
            .fault
            .unwrap_or_else(|| panic!("case {index} produced no fault"));
        assert!(
            fault.contains(expected),
            "case {index} said {fault:?}, wanted {expected:?}"
        );
        assert!(scan.points.is_empty(), "a faulted laser must be dark");
    }
}

/// A runaway script must not take the frame with it. This is the one failure mode that would
/// otherwise hang the visualizer rather than merely darken one fixture.
#[test]
fn a_runaway_script_is_interrupted_and_faulted() {
    const SPIN: &str = "export function scan() { while (true) {} }";
    let mut engine = engine().with_budget(Duration::from_millis(20));
    let started = Instant::now();
    let scan = engine.scan(0, &request(SPIN, 1, &[]));
    assert!(
        scan.fault.is_some(),
        "an interrupted script must report a fault"
    );
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the interrupt did not stop the script"
    );
}

/// Live reload: the same emitter handed different source recompiles, and the new engine runs with
/// a fresh context rather than inheriting the old one's state.
#[test]
fn changed_source_recompiles_and_starts_from_a_clean_context() {
    const FIRST: &str = "let seen = 0;\nexport function scan() { seen += 1; \
        return { points: [{ x: seen, y: 0, amount: 100 }] }; }";
    let mut engine = engine();
    assert_eq!(engine.scan(0, &request(FIRST, 1, &[])).points[0].x, 1.0);
    assert_eq!(engine.scan(0, &request(FIRST, 1, &[])).points[0].x, 2.0);
    // Same key, so no recompile: the state is deliberately kept across frames.
    const SECOND: &str = "let seen = 100;\nexport function scan() { seen += 1; \
        return { points: [{ x: seen, y: 0, amount: 100 }] }; }";
    assert_eq!(
        engine.scan(0, &request(SECOND, 2, &[])).points[0].x,
        101.0,
        "a new source key must recompile into a clean context"
    );
}

/// Two fixtures of the same model must not share a phase. Each patched laser gets its own
/// context, keyed by emitter, or a rig of identical projectors would animate in lockstep.
#[test]
fn two_lasers_running_one_script_keep_separate_state() {
    const COUNTER: &str = "let n = 0;\nexport function scan() { n += 1; \
        return { points: [{ x: n, y: 0, amount: 100 }] }; }";
    let mut engine = engine();
    engine.scan(0, &request(COUNTER, 1, &[]));
    engine.scan(0, &request(COUNTER, 1, &[]));
    let second = engine.scan(1, &request(COUNTER, 1, &[]));
    assert_eq!(
        second.points[0].x, 1.0,
        "the second laser inherited a phase"
    );
}

/// A script may hold state across frames, which is what an animating pattern needs.
#[test]
fn a_script_may_animate_across_frames() {
    const SPIN: &str = "let phase = 0;\nexport function scan(input) { phase += input.elapsed; \
        return { points: [{ x: Math.cos(phase), y: Math.sin(phase), r: 1, amount: 100 }] }; }";
    let mut engine = engine();
    let first = engine.scan(0, &request(SPIN, 1, &[])).points[0].x;
    let second = engine.scan(0, &request(SPIN, 1, &[])).points[0].x;
    assert!(first != second, "the pattern did not advance");
}

/// The compact form exists so a figure of several hundred points does not spend its budget
/// building property maps. It has to mean exactly what the object form means.
#[test]
fn the_compact_point_form_matches_the_object_form() {
    const COMPACT: &str =
        "export function scan() { return { points: [[-0.5, 0.25, 1, 0.5, 0, 40]] }; }";
    let scan = engine().scan(0, &request(COMPACT, 1, &[]));
    assert_eq!(scan.fault, None);
    let point = scan.points[0];
    assert_eq!((point.x, point.y), (-0.5, 0.25));
    assert_eq!(point.colour, [1.0, 0.5, 0.0]);
    assert_eq!(point.dwell, 1.0, "a single point takes the whole scan");
}

/// A NaN deflection becomes a vertex at infinity and takes the entire picture with it, so it has
/// to be rejected at the boundary rather than clamped somewhere downstream.
#[test]
fn a_point_that_is_not_a_number_is_rejected_at_the_boundary() {
    const NAN: &str = "export function scan() { return { points: [{ x: 0/0, y: 0 }] }; }";
    let scan = engine().scan(0, &request(NAN, 1, &[]));
    assert!(
        scan.fault
            .is_some_and(|fault| fault.contains("not a number")),
        "a NaN deflection must be refused"
    );
}

/// A runaway loop that emits points must not be able to turn a typo into gigabytes of geometry.
#[test]
fn an_absurd_point_count_is_refused() {
    let source = format!(
        "export function scan() {{ const p = []; for (let i = 0; i < {}; i++) \
         p.push([0, 0, 1, 1, 1, 1]); return {{ points: p }}; }}",
        MAX_POINTS + 1
    );
    let scan = engine().scan(0, &request(&source, 1, &[]));
    assert!(
        scan.fault.is_some_and(|fault| fault.contains("more than")),
        "an oversized scan must be refused"
    );
}

/// A script gets its fixture's slots and nothing else — no filesystem, no network, no timers.
#[test]
fn a_script_has_no_host_beyond_its_input() {
    for probe in [
        "typeof require",
        "typeof fetch",
        "typeof setTimeout",
        "typeof process",
        "typeof console",
    ] {
        let source = format!(
            "export function scan() {{ if ({probe} !== 'undefined') \
             throw new Error('reachable'); return {{ points: [] }}; }}"
        );
        let scan = engine().scan(0, &request(&source, 1, &[]));
        assert_eq!(scan.fault, None, "{probe} was reachable from a scan script");
    }
}

/// Dropping a laser from the rig must drop its context too, or a show edited all evening leaks a
/// JavaScript context per removed fixture.
#[test]
fn contexts_are_released_when_a_laser_leaves_the_rig() {
    const ANY: &str = "export function scan() { return { points: [] }; }";
    let mut engine = engine();
    engine.scan(0, &request(ANY, 1, &[]));
    engine.scan(1, &request(ANY, 1, &[]));
    assert_eq!(engine.programs.len(), 2);
    engine.retain(|emitter| emitter == 0);
    assert_eq!(engine.programs.len(), 1);
}
