#![forbid(unsafe_code)]
//! Sandboxed package-owned DMX-to-particle Effect programs.

use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Runtime, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use viz_scene::{EffectFrame, ParticleEmitter, ParticleFamily, ParticleTrigger};

pub const DEFAULT_BUDGET: Duration = Duration::from_millis(4);
pub const DEFAULT_MEMORY_LIMIT: usize = 8 * 1024 * 1024;
pub const MAX_EMITTERS: usize = 32;

pub struct EffectRequest<'a> {
    pub source: &'a str,
    pub source_key: u64,
    pub result_version: u16,
    pub slots: &'a [u8],
    pub time_seconds: f64,
    pub elapsed_seconds: f64,
    pub intensity: f32,
    pub fixture_identity: &'a str,
    pub capture_seed: u64,
}

pub struct EffectEngine {
    runtime: Runtime,
    programs: HashMap<usize, Program>,
    deadline: Arc<AtomicU64>,
    epoch: Instant,
    budget: Duration,
}

struct Program {
    context: Context,
    source_key: u64,
    fixture_identity: String,
    compile_fault: Option<String>,
}

impl EffectEngine {
    pub fn new() -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|error| format!("effect runtime: {error}"))?;
        runtime.set_memory_limit(DEFAULT_MEMORY_LIMIT);
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        let epoch = Instant::now();
        let watched = Arc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            epoch.elapsed().as_micros().min(u64::MAX as u128) as u64
                > watched.load(Ordering::Relaxed)
        })));
        Ok(Self {
            runtime,
            programs: HashMap::new(),
            deadline,
            epoch,
            budget: DEFAULT_BUDGET,
        })
    }

    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }
    pub fn retain(&mut self, keep: impl Fn(usize) -> bool) {
        self.programs.retain(|index, _| keep(*index));
    }

    pub fn run(&mut self, index: usize, request: &EffectRequest<'_>) -> EffectFrame {
        if request.result_version != 1 {
            return faulted(format!(
                "unsupported effect result version {}",
                request.result_version
            ));
        }
        if let Err(reason) = self.ensure_program(index, request) {
            return faulted(reason);
        }
        let Some(program) = self.programs.get(&index) else {
            return faulted("effect script was not compiled".into());
        };
        if let Some(reason) = &program.compile_fault {
            return faulted(reason.clone());
        }
        self.arm();
        let outcome = program.context.with(|ctx| invoke(&ctx, request));
        self.disarm();
        outcome.unwrap_or_else(faulted)
    }

    fn ensure_program(&mut self, index: usize, request: &EffectRequest<'_>) -> Result<(), String> {
        if self.programs.get(&index).is_some_and(|program| {
            program.source_key == request.source_key
                && program.fixture_identity == request.fixture_identity
        }) {
            return Ok(());
        }
        let context = Context::full(&self.runtime)
            .map_err(|error| format!("effect context could not be created: {error}"))?;
        self.arm();
        let compile_fault = context.with(|ctx| declare(&ctx, request.source)).err();
        self.disarm();
        self.programs.insert(
            index,
            Program {
                context,
                source_key: request.source_key,
                fixture_identity: request.fixture_identity.to_owned(),
                compile_fault,
            },
        );
        Ok(())
    }

    fn arm(&self) {
        let now = self.epoch.elapsed().as_micros().min(u64::MAX as u128) as u64;
        self.deadline.store(
            now.saturating_add(self.budget.as_micros() as u64),
            Ordering::Relaxed,
        );
    }
    fn disarm(&self) {
        self.deadline.store(u64::MAX, Ordering::Relaxed);
    }
}

fn declare(ctx: &Ctx<'_>, source: &str) -> Result<(), String> {
    let (module, promise) = rquickjs::Module::declare(ctx.clone(), "effect", source)
        .catch(ctx)
        .map_err(|error| format!("effect script could not be compiled: {error}"))?
        .eval()
        .catch(ctx)
        .map_err(|error| format!("effect script failed while loading: {error}"))?;
    promise
        .finish::<()>()
        .catch(ctx)
        .map_err(|error| format!("effect script failed while loading: {error}"))?;
    let function: Value = module
        .get("effect")
        .ok()
        .unwrap_or_else(|| Value::new_undefined(ctx.clone()));
    if !function.is_function() {
        return Err("effect script does not export an `effect` function".into());
    }
    ctx.globals()
        .set("__effect", function)
        .catch(ctx)
        .map_err(|error| format!("effect script could not be prepared: {error}"))
}

fn invoke(ctx: &Ctx<'_>, request: &EffectRequest<'_>) -> Result<EffectFrame, String> {
    let function: Function = ctx
        .globals()
        .get("__effect")
        .catch(ctx)
        .map_err(|_| "effect script does not export an `effect` function".to_string())?;
    let input = Object::new(ctx.clone())
        .catch(ctx)
        .map_err(|error| format!("effect input could not be built: {error}"))?;
    let dmx = rquickjs::Array::new(ctx.clone())
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    for (index, value) in request.slots.iter().enumerate() {
        dmx.set(index, *value)
            .catch(ctx)
            .map_err(|error| error.to_string())?;
    }
    input
        .set("dmx", dmx)
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    input
        .set("time", request.time_seconds)
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    input
        .set("elapsed", request.elapsed_seconds)
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    input
        .set("intensity", request.intensity)
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    input
        .set("fixtureId", request.fixture_identity)
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    input
        .set("seed", request.capture_seed.to_string())
        .catch(ctx)
        .map_err(|error| error.to_string())?;
    let output: Value = function.call((input,)).catch(ctx).map_err(|error| {
        let message = error.to_string();
        if message.trim().is_empty() || message.contains("interrupted") {
            "effect script exceeded its time budget".into()
        } else {
            format!("effect script failed: {message}")
        }
    })?;
    read_output(&output, request.slots)
}

fn read_output(output: &Value<'_>, slots: &[u8]) -> Result<EffectFrame, String> {
    if output.is_null() || output.is_undefined() {
        return Ok(EffectFrame {
            version: 1,
            slots: slots.to_vec(),
            ..Default::default()
        });
    }
    let object = output
        .as_object()
        .ok_or_else(|| "effect must return an object".to_string())?;
    let version = object.get::<_, u16>("version").unwrap_or(1);
    if version != 1 {
        return Err(format!("unsupported effect result version {version}"));
    }
    let emitters = object
        .get::<_, Value>("emitters")
        .map_err(|_| "effect result has no `emitters`".to_string())?;
    let array = emitters
        .as_array()
        .ok_or_else(|| "effect result `emitters` is not an array".to_string())?;
    if array.len() > MAX_EMITTERS {
        return Err(format!(
            "effect returned {} emitters, more than {MAX_EMITTERS}",
            array.len()
        ));
    }
    let mut result = Vec::with_capacity(array.len());
    for (index, value) in array.iter::<Value>().enumerate() {
        let value = value.map_err(|error| format!("effect emitter {index}: {error}"))?;
        result.push(read_emitter(
            value
                .as_object()
                .ok_or_else(|| format!("effect emitter {index} is not an object"))?,
            index,
        )?);
    }
    Ok(EffectFrame {
        version,
        timeline_seconds: 0.0,
        emitters: result,
        slots: slots.to_vec(),
        fault: None,
    })
}

fn read_emitter(object: &Object<'_>, index: usize) -> Result<ParticleEmitter, String> {
    let family = match text(object, "family", index)?.as_str() {
        "flame" => ParticleFamily::Flame,
        "spark" => ParticleFamily::Spark,
        other => {
            return Err(format!(
                "effect emitter {index} has unsupported family `{other}`"
            ));
        }
    };
    let trigger = match object
        .get::<_, String>("state")
        .unwrap_or_else(|_| "hold".into())
        .as_str()
    {
        "off" => ParticleTrigger::Off,
        "trigger" => ParticleTrigger::Trigger,
        "hold" => ParticleTrigger::Hold,
        "release" => ParticleTrigger::Release,
        "retrigger" => ParticleTrigger::Retrigger,
        other => {
            return Err(format!(
                "effect emitter {index} has unsupported state `{other}`"
            ));
        }
    };
    let origin = vector(object, "origin", index, [0.0, 0.0, 0.0])?;
    let mut direction = vector(object, "direction", index, [0.0, 1.0, 0.0])?;
    let length = direction
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if length <= f32::EPSILON {
        return Err(format!("effect emitter {index} direction is zero"));
    }
    for value in &mut direction {
        *value /= length;
    }
    Ok(ParticleEmitter {
        family,
        origin,
        direction,
        width_metres: bounded(object, "width", index, 0.01, 5.0)?,
        reach_metres: bounded(object, "height", index, 0.01, 20.0)?,
        intensity: bounded(object, "intensity", index, 0.0, 1.0)?,
        density: bounded(object, "density", index, 0.0, 1.0)?,
        lifetime_seconds: bounded(object, "lifetime", index, 0.02, 30.0)?,
        colour: vector(
            object,
            "color",
            index,
            match family {
                ParticleFamily::Flame => [1.0, 0.18, 0.02],
                _ => [1.0, 0.65, 0.12],
            },
        )?
        .map(|value| value.clamp(0.0, 1.0)),
        trigger,
    })
}

fn text(object: &Object<'_>, field: &str, index: usize) -> Result<String, String> {
    object
        .get(field)
        .map_err(|_| format!("effect emitter {index} has no `{field}`"))
}
fn bounded(
    object: &Object<'_>,
    field: &str,
    index: usize,
    min: f32,
    max: f32,
) -> Result<f32, String> {
    let value = object
        .get::<_, f64>(field)
        .map_err(|_| format!("effect emitter {index} has no `{field}`"))? as f32;
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!(
            "effect emitter {index} `{field}` must be {min}..={max}"
        ));
    }
    Ok(value)
}
fn vector(
    object: &Object<'_>,
    field: &str,
    index: usize,
    fallback: [f32; 3],
) -> Result<[f32; 3], String> {
    let Ok(value) = object.get::<_, Value>(field) else {
        return Ok(fallback);
    };
    if value.is_undefined() || value.is_null() {
        return Ok(fallback);
    }
    let array = value
        .as_array()
        .ok_or_else(|| format!("effect emitter {index} `{field}` is not an array"))?;
    if array.len() != 3 {
        return Err(format!(
            "effect emitter {index} `{field}` needs three numbers"
        ));
    }
    let mut result = [0.0; 3];
    for (slot, out) in result.iter_mut().enumerate() {
        *out = array
            .get::<f64>(slot)
            .map_err(|_| format!("effect emitter {index} `{field}` contains a non-number"))?
            as f32;
        if !out.is_finite() {
            return Err(format!(
                "effect emitter {index} `{field}` contains a non-finite number"
            ));
        }
    }
    Ok(result)
}
fn faulted(reason: String) -> EffectFrame {
    EffectFrame {
        fault: Some(reason),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCRIPT: &str = r#"export function effect(input) { return { version: 1, emitters: [{ family: 'spark', origin: [0,0,0], direction: [0,2,0], width: .2, height: 3, intensity: input.intensity, density: .7, lifetime: 2.5, color: [1,.5,.1], state: input.dmx[0] ? 'trigger' : 'off' }] }; }"#;
    #[test]
    fn package_program_receives_typed_input_and_returns_a_bounded_spark() {
        let mut engine = EffectEngine::new().unwrap();
        let frame = engine.run(
            0,
            &EffectRequest {
                source: SCRIPT,
                source_key: 1,
                result_version: 1,
                slots: &[255],
                time_seconds: 2.0,
                elapsed_seconds: 0.016,
                intensity: 0.8,
                fixture_identity: "fixture",
                capture_seed: 42,
            },
        );
        assert_eq!(frame.fault, None);
        assert_eq!(frame.emitters.len(), 1);
        assert_eq!(frame.emitters[0].family, ParticleFamily::Spark);
        assert_eq!(frame.emitters[0].lifetime_seconds, 2.5);
        assert_eq!(frame.emitters[0].direction, [0.0, 1.0, 0.0]);
    }
    #[test]
    fn one_fault_is_contained_to_that_fixture() {
        let mut engine = EffectEngine::new().unwrap();
        let request = |source| EffectRequest {
            source,
            source_key: source.len() as u64,
            result_version: 1,
            slots: &[],
            time_seconds: 0.0,
            elapsed_seconds: 0.0,
            intensity: 1.0,
            fixture_identity: "fixture",
            capture_seed: 1,
        };
        assert!(
            engine
                .run(
                    0,
                    &request("export function effect(){throw new Error('bad')}")
                )
                .fault
                .is_some()
        );
        assert_eq!(engine.run(1, &request(SCRIPT)).fault, None);
    }

    #[test]
    fn the_sandbox_exposes_no_host_capabilities() {
        let source = "export function effect(){ if(typeof fetch!=='undefined'||typeof require!=='undefined'||typeof setTimeout!=='undefined'||typeof console!=='undefined') throw new Error('host leaked'); return {emitters:[]}; }";
        let mut engine = EffectEngine::new().unwrap();
        let frame = engine.run(
            0,
            &EffectRequest {
                source,
                source_key: 9,
                result_version: 1,
                slots: &[],
                time_seconds: 0.0,
                elapsed_seconds: 0.0,
                intensity: 1.0,
                fixture_identity: "fixture",
                capture_seed: 1,
            },
        );
        assert_eq!(frame.fault, None);
    }

    #[test]
    fn the_program_receives_exact_time_identity_seed_slots_and_decoded_intensity() {
        let source = r#"export function effect(input) {
          if (input.dmx.join(',') !== '3,17,255' || input.time !== 12.5 || input.elapsed !== 0.25 ||
              input.intensity !== 0.75 || input.fixtureId !== 'fixture-42' || input.seed !== '99') {
            throw new Error('input contract mismatch');
          }
          return {emitters: []};
        }"#;
        let mut engine = EffectEngine::new().unwrap();
        let frame = engine.run(
            0,
            &EffectRequest {
                source,
                source_key: 10,
                result_version: 1,
                slots: &[3, 17, 255],
                time_seconds: 12.5,
                elapsed_seconds: 0.25,
                intensity: 0.75,
                fixture_identity: "fixture-42",
                capture_seed: 99,
            },
        );
        assert_eq!(frame.fault, None, "{:?}", frame.fault);
        assert_eq!(frame.slots, [3, 17, 255]);
    }

    #[test]
    fn package_imports_are_rejected_without_poisoning_an_independent_fixture() {
        let mut engine = EffectEngine::new().unwrap();
        let imported = EffectRequest {
            source: "import value from 'host'; export function effect(){return value}",
            source_key: 11,
            result_version: 1,
            slots: &[],
            time_seconds: 0.0,
            elapsed_seconds: 0.0,
            intensity: 1.0,
            fixture_identity: "importer",
            capture_seed: 1,
        };
        assert!(engine.run(0, &imported).fault.is_some());
        let independent = EffectRequest {
            source: SCRIPT,
            source_key: 12,
            fixture_identity: "good",
            slots: &[1],
            ..imported
        };
        assert_eq!(engine.run(1, &independent).fault, None);
    }

    #[test]
    fn runaway_and_oversized_programs_fault_without_poisoning_the_next_fixture() {
        let mut engine = EffectEngine::new()
            .unwrap()
            .with_budget(Duration::from_millis(1));
        let request = |source, key| EffectRequest {
            source,
            source_key: key,
            result_version: 1,
            slots: &[],
            time_seconds: 0.0,
            elapsed_seconds: 0.0,
            intensity: 1.0,
            fixture_identity: "fixture",
            capture_seed: 1,
        };
        assert!(
            engine
                .run(0, &request("export function effect(){while(true){}}", 1))
                .fault
                .is_some()
        );
        let oversized = "export function effect(){return {emitters:Array.from({length:33},()=>({family:'flame',width:.1,height:1,intensity:1,density:1,lifetime:1}))}}";
        assert!(
            engine
                .run(1, &request(oversized, 2))
                .fault
                .unwrap()
                .contains("more than 32")
        );
        assert_eq!(engine.run(2, &request(SCRIPT, 3)).fault, None);
    }

    #[test]
    fn identical_profiles_keep_isolated_script_state() {
        let source = "let calls=0; export function effect(){calls++; return {emitters:[{family:'flame',width:.1,height:1,intensity:calls/10,density:1,lifetime:1}]}}";
        let mut engine = EffectEngine::new().unwrap();
        let request = EffectRequest {
            source,
            source_key: 1,
            result_version: 1,
            slots: &[],
            time_seconds: 0.0,
            elapsed_seconds: 0.0,
            intensity: 1.0,
            fixture_identity: "fixture",
            capture_seed: 1,
        };
        let first = engine.run(0, &request);
        assert_eq!(first.fault, None, "{:?}", first.fault);
        assert_eq!(first.emitters[0].intensity, 0.1);
        assert_eq!(engine.run(0, &request).emitters[0].intensity, 0.2);
        assert_eq!(engine.run(1, &request).emitters[0].intensity, 0.1);
        let replacement = EffectRequest {
            fixture_identity: "replacement",
            ..request
        };
        assert_eq!(
            engine.run(0, &replacement).emitters[0].intensity,
            0.1,
            "an emitter index reused by a different fixture gets fresh state"
        );
        let mut restarted = EffectEngine::new().unwrap();
        assert_eq!(
            restarted.run(0, &replacement).emitters[0].intensity,
            0.1,
            "a live renderer restart begins fresh from current input"
        );
    }
}
