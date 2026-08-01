#![forbid(unsafe_code)]
//! The scan engine a laser fixture projects with.
//!
//! Every other fixture class can be described by parameters: an angle, a colour, a gobo slot. A
//! laser cannot. Two projectors handed identical DMX draw completely different pictures, because
//! what an audience sees is decided by a pattern engine inside the fixture and the DMX only picks
//! between its presets and modulates them. Enumerating the results is hopeless — a mid-range
//! projector ships hundreds of patterns, and no two manufacturers agree on any of them.
//!
//! So a laser profile carries its engine as source text and the visualizer runs it. Once per
//! displayed frame, per laser, the fixture's script is handed its own DMX slots and returns the
//! path the beam actually takes: a list of control points, each with a deflection, a colour, and
//! the share of the scan spent getting there. Everything downstream — the geometry, the dwell
//! weighting that makes corners bright, the persistence that makes a 30 kpps figure read as a
//! solid line — works from that one list and needs to know nothing about the fixture.
//!
//! # The contract
//!
//! A script is an ES module exporting `scan`:
//!
//! ```javascript
//! export function scan(input) {
//!   return {
//!     points: [
//!       { x: -1, y: 0, r: 1, g: 0, b: 0, amount: 50 },
//!       { x:  1, y: 0, r: 1, g: 0, b: 0, amount: 50 },
//!     ],
//!   };
//! }
//! ```
//!
//! `x` and `y` are deflections in `-1..=1` of the scanner's half angle, `r`/`g`/`b` are `0..=1`,
//! and `amount` is the percentage of one complete scan spent reaching that point. A point may also
//! be given as a compact `[x, y, r, g, b, amount]` array, which is worth it once a figure runs to
//! hundreds of points. The module may hold state between calls: a laser animating a pattern needs
//! to, and each patched fixture gets its own isolated context so two of the same model do not
//! share a phase.
//!
//! # What a script may not do
//!
//! There is no host beyond `input`. No filesystem, no network, no timers, no console, no imports.
//! A script gets a fixed slice of wall-clock time per call and is interrupted if it exceeds it,
//! and it runs against a memory ceiling. A script that fails to compile, throws, returns something
//! unusable, or overruns leaves its laser dark with a stated reason — never silently, and never
//! taking the frame down with it.

use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Runtime, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use viz_scene::{LaserScan, ScanPoint};

mod convert;

#[cfg(test)]
mod tests;

use convert::read_output;

/// How long one laser's script may run before it is interrupted.
///
/// Generous for the work involved — a scan engine is arithmetic over a few hundred points — and
/// still small enough that every laser in a large rig can miss its budget without the frame rate
/// collapsing. A script that hits this is faulted rather than throttled: it is broken, and saying
/// so once is more useful than quietly halving the frame rate forever.
pub const DEFAULT_BUDGET: Duration = Duration::from_millis(4);

/// Memory ceiling for one laser's context. A scan engine that needs more than this is not
/// computing a scan path.
pub const DEFAULT_MEMORY_LIMIT: usize = 8 * 1024 * 1024;

/// The most points one scan may contain.
///
/// A 30 kpps scanner running a 30 Hz figure is a thousand points, so this is a wide margin over
/// anything physical. It exists because the point list becomes render geometry, and a script with
/// a runaway loop must not be able to turn a typo into gigabytes of vertices.
pub const MAX_POINTS: usize = 16_384;

/// What a laser needs to run its scan for one frame.
pub struct ScanRequest<'a> {
    /// The script source. Compared by [`Self::source_key`] rather than by content.
    pub source: &'a str,
    /// Identifies this exact source text. A change here recompiles the script, which is the seam
    /// live reload works through: rewrite the file, hand over a new key, and the next frame runs
    /// the new engine with a fresh context.
    pub source_key: u64,
    /// The fixture's own DMX slots, in patch order from its start address.
    pub slots: &'a [u8],
    /// Seconds since the visualizer started. Monotonic, and the same value every laser in one
    /// frame is given.
    pub time_seconds: f64,
    /// Seconds since this laser's previous scan, for a script integrating its own motion.
    pub elapsed_seconds: f64,
    /// The decoded master intensity, `0..=1`, for a script that would rather not find the dimmer
    /// in its own slots.
    pub intensity: f32,
}

/// Runs laser scan scripts, one isolated context per emitter.
pub struct ScanEngine {
    runtime: Runtime,
    programs: HashMap<usize, Program>,
    /// Deadline for the call in progress, in microseconds since [`Self::epoch`]. Read by the
    /// interrupt handler, which is the only thing that can stop a runaway script.
    deadline: Arc<AtomicU64>,
    epoch: Instant,
    budget: Duration,
}

struct Program {
    context: Context,
    source_key: u64,
    /// Set when the script could not be compiled. Kept so a broken script is reported every frame
    /// without being recompiled every frame.
    compile_fault: Option<String>,
}

impl ScanEngine {
    pub fn new() -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|error| format!("scan runtime: {error}"))?;
        runtime.set_memory_limit(DEFAULT_MEMORY_LIMIT);
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        let epoch = Instant::now();
        let watched = Arc::clone(&deadline);
        let started = epoch;
        // The only way out of `while (true) {}`. QuickJS calls this periodically and abandons the
        // script when it answers true, which surfaces as an ordinary exception at the call site.
        runtime.set_interrupt_handler(Some(Box::new(move || {
            let elapsed = started.elapsed().as_micros().min(u64::MAX as u128) as u64;
            elapsed > watched.load(Ordering::Relaxed)
        })));
        Ok(Self {
            runtime,
            programs: HashMap::new(),
            deadline,
            epoch,
            budget: DEFAULT_BUDGET,
        })
    }

    /// Override the per-call time budget. Mainly for tests, which need a budget short enough to
    /// prove the interrupt fires without making the suite slow.
    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    /// Drop the context for an emitter that no longer exists, so a rig edited during a show does
    /// not leak a JavaScript context per removed laser.
    pub fn retain(&mut self, keep: impl Fn(usize) -> bool) {
        self.programs.retain(|emitter, _| keep(*emitter));
    }

    /// Run one laser's scan for this frame.
    ///
    /// Never fails: a script that cannot be compiled or run produces an empty scan carrying the
    /// reason, which is what puts the fault in front of the operator instead of in a log.
    pub fn scan(&mut self, emitter: usize, request: &ScanRequest<'_>) -> LaserScan {
        if let Err(reason) = self.ensure_program(emitter, request) {
            return faulted(reason);
        }
        let Some(program) = self.programs.get(&emitter) else {
            return faulted("scan script was not compiled".into());
        };
        if let Some(fault) = &program.compile_fault {
            return faulted(fault.clone());
        }
        self.arm_deadline();
        let outcome = program.context.with(|ctx| run_scan(&ctx, request));
        self.disarm_deadline();
        match outcome {
            Ok(scan) => scan,
            Err(reason) => faulted(reason),
        }
    }

    /// Compile the script if this emitter has none, or if its source changed.
    fn ensure_program(&mut self, emitter: usize, request: &ScanRequest<'_>) -> Result<(), String> {
        if self
            .programs
            .get(&emitter)
            .is_some_and(|program| program.source_key == request.source_key)
        {
            return Ok(());
        }
        let context = Context::full(&self.runtime)
            .map_err(|error| format!("scan context could not be created: {error}"))?;
        self.arm_deadline();
        // A module's top level is script too, and an infinite loop there has to be caught by the
        // same deadline that guards the call itself.
        let compile_fault = context
            .with(|ctx| declare_module(&ctx, request.source))
            .err();
        self.disarm_deadline();
        self.programs.insert(
            emitter,
            Program {
                context,
                source_key: request.source_key,
                compile_fault,
            },
        );
        Ok(())
    }

    fn arm_deadline(&self) {
        let elapsed = self.epoch.elapsed().as_micros().min(u64::MAX as u128) as u64;
        self.deadline.store(
            elapsed.saturating_add(self.budget.as_micros() as u64),
            Ordering::Relaxed,
        );
    }

    fn disarm_deadline(&self) {
        self.deadline.store(u64::MAX, Ordering::Relaxed);
    }
}

fn faulted(reason: String) -> LaserScan {
    LaserScan {
        points: Vec::new(),
        points_per_second: 0.0,
        slots: Vec::new(),
        fault: Some(reason),
    }
}

/// Evaluate the module and keep its `scan` export on the context's globals.
///
/// Reaching the export through a global rather than holding the module handle keeps the call path
/// free of lifetimes tied to the compile, which matters because the two happen on different
/// frames.
fn declare_module(ctx: &Ctx<'_>, source: &str) -> Result<(), String> {
    let (module, promise) = rquickjs::Module::declare(ctx.clone(), "scan", source)
        .catch(ctx)
        .map_err(|error| format!("scan script could not be compiled: {error}"))?
        .eval()
        .catch(ctx)
        .map_err(|error| format!("scan script failed while loading: {error}"))?;
    promise
        .finish::<()>()
        .catch(ctx)
        .map_err(|error| format!("scan script failed while loading: {error}"))?;
    // A module with no such export reads back as `undefined` rather than failing, so the missing
    // case and the wrong-type case have to be told apart here or they collapse into one message
    // that answers neither question.
    let scan: Value = module
        .get("scan")
        .ok()
        .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
    if scan.is_undefined() {
        return Err("scan script does not export a `scan` function".into());
    }
    if !scan.is_function() {
        return Err("the scan script's `scan` export is not a function".into());
    }
    ctx.globals()
        .set("__scan", scan)
        .catch(ctx)
        .map_err(|error| format!("scan script could not be prepared: {error}"))?;
    Ok(())
}

fn run_scan(ctx: &Ctx<'_>, request: &ScanRequest<'_>) -> Result<LaserScan, String> {
    let scan: Function = ctx
        .globals()
        .get("__scan")
        .catch(ctx)
        .map_err(|_| "scan script does not export a `scan` function".to_string())?;
    let input = build_input(ctx, request)?;
    let output: Value = scan
        .call((input,))
        .catch(ctx)
        .map_err(|error| describe_call_failure(&error.to_string()))?;
    read_output(ctx, &output)
}

/// The interrupt surfaces as an ordinary exception with no message, which would reach the operator
/// as a blank fault. Name it for what it is.
fn describe_call_failure(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() || trimmed == "Error" || trimmed.contains("interrupted") {
        return format!(
            "scan script exceeded its {} ms time budget",
            DEFAULT_BUDGET.as_millis()
        );
    }
    format!("scan script failed: {trimmed}")
}

fn build_input<'js>(ctx: &Ctx<'js>, request: &ScanRequest<'_>) -> Result<Object<'js>, String> {
    let input = Object::new(ctx.clone())
        .catch(ctx)
        .map_err(|error| format!("scan input could not be built: {error}"))?;
    let slots = rquickjs::Array::new(ctx.clone())
        .catch(ctx)
        .map_err(|error| format!("scan input could not be built: {error}"))?;
    for (index, slot) in request.slots.iter().enumerate() {
        slots
            .set(index, *slot)
            .catch(ctx)
            .map_err(|error| format!("scan input could not be built: {error}"))?;
    }
    let set = |name: &str, value: Value<'js>| -> Result<(), String> {
        input
            .set(name, value)
            .catch(ctx)
            .map_err(|error| format!("scan input could not be built: {error}"))
    };
    set("dmx", slots.into_value())?;
    set("time", Value::new_float(ctx.clone(), request.time_seconds))?;
    set(
        "elapsed",
        Value::new_float(ctx.clone(), request.elapsed_seconds),
    )?;
    set(
        "intensity",
        Value::new_float(ctx.clone(), request.intensity as f64),
    )?;
    Ok(input)
}

/// Normalise a script's point list into a renderable scan.
///
/// A script is not required to make its percentages add up — most will emit `100 / n` per point
/// and let rounding fall where it may, and a generated figure may well emit nothing at all. The
/// shares are rescaled to sum to one so the dwell weighting downstream is a true fraction of the
/// frame's light whatever the script did, and a path whose shares are all zero is treated as an
/// even sweep, which is what a script that never thought about timing meant.
pub(crate) fn normalise(points: &mut [ScanPoint]) {
    if points.is_empty() {
        return;
    }
    let total: f32 = points.iter().map(|point| point.dwell.max(0.0)).sum();
    if total <= f32::EPSILON {
        let even = 1.0 / points.len() as f32;
        for point in points.iter_mut() {
            point.dwell = even;
        }
        return;
    }
    for point in points.iter_mut() {
        point.dwell = point.dwell.max(0.0) / total;
    }
}
