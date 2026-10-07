//! Per-fixture projection of one render: resolve each patched fixture's heads and write them to
//! every destination it is patched to.
//!
//! TL-639 round 5: resolving a fixture reads only the frame's immutable inputs and writes only its
//! own output, so a large rig resolves its fixtures on several threads first and then writes
//! visualization values, physical readouts and DMX in fixture order on the caller. Both passes run
//! the same per-fixture control flow ([`project_fixture`]) — the first only resolves, the second
//! replays those outputs and writes — so the bytes, the overlap order of shared universes, the
//! visualization map's insert history and the first error are those of the single-threaded loop.

use std::collections::HashMap;

use light_core::Universe;

use super::{
    AxisInversion, EngineError, RuntimeGeneration, encode_profile_split, resolve_profile_fixture,
};
use crate::ResolvedProfileFixtureOutput;

/// Below this many fixtures a render resolves on the caller: spawning threads costs more than
/// the work (the TL-564 mix renders about a hundred).
/// Engine tests render in parallel from two fixtures, so they all prove the two passes equal the
/// single-threaded loop.
pub(crate) const PARALLEL_RENDER_MIN_FIXTURES: usize = if cfg!(test) { 2 } else { 512 };
/// Fixtures per chunk at least, so a chunk amortises its claim.
const MIN_FIXTURES_PER_CHUNK: usize = if cfg!(test) { 1 } else { 32 };

/// Everything a fixture's projection reads and none of what it writes. Bundled because the render
/// resolves each of these once for the whole frame, and threading seven borrows through one call
/// per fixture said nothing the frame did not already say.
pub(crate) struct ProjectionInputs<'a> {
    pub position_native: &'a crate::native_position_projection::NativePositionProjection,
    pub values: &'a crate::ProfileValueIndex<'a>,
    pub options: crate::RenderOptions,
    pub highlight_layers:
        &'a HashMap<light_core::FixtureId, light_programmer::HighlightOutputLayer>,
    pub highlight_look: &'a light_fixture::HighlightLook,
}

/// What a render writes per fixture.
pub(crate) struct ProjectionWrites<'w> {
    pub universes: &'w mut HashMap<Universe, light_output::DmxFrame>,
    pub patched_slots: &'w mut HashMap<Universe, u16>,
    pub visualization: &'w mut crate::ResolvedValues,
    pub physical: &'w mut crate::PhysicalForwardFrame,
}

/// Resolved outputs of one chunk of fixtures, in the order the fixtures' projection asks for
/// them, kept between frames so their vectors are grown once.
#[derive(Default)]
pub(crate) struct ResolvedChunk {
    outputs: Vec<ResolvedProfileFixtureOutput>,
    used: usize,
    /// The first resolve error of the chunk; nothing after it was resolved.
    error: Option<EngineError>,
}

/// Per-chunk resolved outputs, kept by the engine between frames.
#[derive(Default)]
pub(crate) struct RenderChunks {
    chunks: Vec<parking_lot::Mutex<ResolvedChunk>>,
    workers: Vec<()>,
}

/// Project every fixture of the frame, in fixture order.
pub(crate) fn project_fixtures(
    generation: &RuntimeGeneration,
    inputs: &ProjectionInputs<'_>,
    output: &mut ResolvedProfileFixtureOutput,
    writes: &mut ProjectionWrites<'_>,
    pool: Option<&crate::parallel::OutputPool>,
    chunks: &parking_lot::Mutex<RenderChunks>,
) -> Result<(), EngineError> {
    let fixtures = &generation.snapshot().fixtures;
    let Some(pool) = pool.filter(|_| fixtures.len() >= PARALLEL_RENDER_MIN_FIXTURES) else {
        let mut pass = ResolveInto { output };
        for fixture in fixtures.iter() {
            project_fixture(fixture, generation, inputs, &mut pass, Some(writes))?;
        }
        return Ok(());
    };
    let workers = pool.workers();
    let mut chunks = chunks.lock();
    let RenderChunks {
        chunks,
        workers: worker_slots,
    } = &mut *chunks;
    let count = crate::parallel::chunk_count(fixtures.len(), workers, MIN_FIXTURES_PER_CHUNK);
    chunks.resize_with(count, Default::default);
    worker_slots.resize(workers, ());
    let chunks = &chunks[..count];
    crate::parallel::run_ordered(Some(pool), worker_slots, count, |_, chunk| {
        let mut resolved = chunks[chunk].lock();
        let resolved = &mut *resolved;
        resolved.used = 0;
        resolved.error = None;
        let mut pass = Record { chunk: resolved };
        for fixture in &fixtures[crate::parallel::chunk_range(fixtures.len(), count, chunk)] {
            if let Err(error) = project_fixture(fixture, generation, inputs, &mut pass, None) {
                pass.chunk.error = Some(error);
                break;
            }
        }
    });
    for (index, chunk) in chunks.iter().enumerate() {
        let mut chunk = chunk.lock();
        let mut pass = Replay {
            chunk: &mut chunk,
            next: 0,
        };
        for fixture in &fixtures[crate::parallel::chunk_range(fixtures.len(), count, index)] {
            project_fixture(fixture, generation, inputs, &mut pass, Some(writes))?;
        }
    }
    Ok(())
}

/// How one pass of [`project_fixture`] obtains each resolved output it asks for.
trait FixturePass {
    fn resolve(
        &mut self,
        resolve: &dyn Fn(&mut ResolvedProfileFixtureOutput) -> Result<(), EngineError>,
    ) -> Result<(), EngineError>;
    /// The output of the latest `resolve`.
    fn output(&self) -> &ResolvedProfileFixtureOutput;
}

/// The single-threaded pass: resolve into one reused buffer.
struct ResolveInto<'o> {
    output: &'o mut ResolvedProfileFixtureOutput,
}

impl FixturePass for ResolveInto<'_> {
    fn resolve(
        &mut self,
        resolve: &dyn Fn(&mut ResolvedProfileFixtureOutput) -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        resolve(self.output)
    }

    fn output(&self) -> &ResolvedProfileFixtureOutput {
        self.output
    }
}

/// The worker pass: resolve each output into the chunk's next slot.
struct Record<'c> {
    chunk: &'c mut ResolvedChunk,
}

impl FixturePass for Record<'_> {
    fn resolve(
        &mut self,
        resolve: &dyn Fn(&mut ResolvedProfileFixtureOutput) -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        if self.chunk.used == self.chunk.outputs.len() {
            self.chunk.outputs.push(Default::default());
        }
        resolve(&mut self.chunk.outputs[self.chunk.used])?;
        self.chunk.used += 1;
        Ok(())
    }

    fn output(&self) -> &ResolvedProfileFixtureOutput {
        &self.chunk.outputs[self.chunk.used - 1]
    }
}

/// The writing pass: hand back the recorded outputs in order, and the recorded error where the
/// worker stopped.
struct Replay<'c> {
    chunk: &'c mut ResolvedChunk,
    next: usize,
}

impl FixturePass for Replay<'_> {
    fn resolve(
        &mut self,
        _: &dyn Fn(&mut ResolvedProfileFixtureOutput) -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        if self.next == self.chunk.used {
            return Err(self.chunk.error.take().unwrap_or_else(|| {
                EngineError::Invalid("parallel render replayed past its resolved outputs".into())
            }));
        }
        self.next += 1;
        Ok(())
    }

    fn output(&self) -> &ResolvedProfileFixtureOutput {
        &self.chunk.outputs[self.next - 1]
    }
}

/// Resolve one patched fixture and, with `writes`, write it to every destination it is patched
/// to. Without `writes` only the resolves run.
fn project_fixture(
    fixture: &light_fixture::PatchedFixture,
    generation: &RuntimeGeneration,
    inputs: &ProjectionInputs<'_>,
    pass: &mut impl FixturePass,
    mut writes: Option<&mut ProjectionWrites<'_>>,
) -> Result<(), EngineError> {
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or_else(|| {
            EngineError::Invalid("schema-v2 fixture is missing its profile snapshot".into())
        })?;
    let mode_id = fixture.definition.mode_id.ok_or_else(|| {
        EngineError::Invalid("schema-v2 fixture is missing its mode identity".into())
    })?;
    let mode = profile
        .mode(mode_id)
        .ok_or_else(|| EngineError::Invalid("schema-v2 fixture mode is missing".into()))?;
    let projection = generation
        .profile_projection(fixture.fixture_id)
        .ok_or_else(|| {
            EngineError::Invalid("schema-v2 fixture projection plan is missing".into())
        })?;
    let job = |inversion: AxisInversion, instance: uuid::Uuid| {
        move |output: &mut ResolvedProfileFixtureOutput| {
            resolve_profile_fixture(
                fixture,
                mode,
                projection,
                None,
                inputs.values,
                inputs.options,
                inputs.highlight_layers,
                inputs.highlight_look,
                inversion,
                instance,
                inputs
                    .position_native
                    .instance(fixture.fixture_id, instance),
                output,
            )
        }
    };
    let physical = generation.physical_projection();
    if profile.patch_policy != light_fixture::PatchPolicy::Dmx {
        pass.resolve(&job(AxisInversion::default(), fixture.fixture_id.0))?;
        if let Some(writes) = writes.as_deref_mut() {
            insert_profile_visualization_values(writes.visualization, pass.output());
            insert_raw_channel_values(writes.visualization, fixture, mode, pass.output());
            physical.evaluate(
                fixture.fixture_id,
                0,
                &pass.output().channels,
                writes.physical,
            )?;
        }
        for (index, copy) in fixture.multipatch.iter().enumerate() {
            if inputs
                .position_native
                .instance(fixture.fixture_id, copy.id)
                .is_some()
            {
                pass.resolve(&job(AxisInversion::default(), copy.id))?;
            }
            if let Some(writes) = writes.as_deref_mut() {
                physical.evaluate(
                    fixture.fixture_id,
                    index + 1,
                    &pass.output().channels,
                    writes.physical,
                )?;
            }
        }
        return Ok(());
    }
    let encoding = generation
        .profile_encoding(fixture.fixture_id)
        .ok_or_else(|| EngineError::Invalid("schema-v2 fixture encoding plan is missing".into()))?;
    pass.resolve(&job(
        AxisInversion {
            pan: fixture.invert_pan,
            tilt: fixture.invert_tilt,
        },
        fixture.fixture_id.0,
    ))?;
    if let Some(writes) = writes.as_deref_mut() {
        insert_profile_visualization_values(writes.visualization, pass.output());
        physical.evaluate(
            fixture.fixture_id,
            0,
            &pass.output().channels,
            writes.physical,
        )?;
        encode_profile_destination(
            &fixture.split_patches,
            fixture.universe,
            fixture.address,
            encoding,
            pass.output(),
            writes,
        )?;
    }
    for (index, instance) in fixture.multipatch.iter().enumerate() {
        pass.resolve(&job(
            AxisInversion {
                pan: instance.invert_pan,
                tilt: instance.invert_tilt,
            },
            instance.id,
        ))?;
        if let Some(writes) = writes.as_deref_mut() {
            physical.evaluate(
                fixture.fixture_id,
                index + 1,
                &pass.output().channels,
                writes.physical,
            )?;
            encode_profile_destination(
                &instance.split_patches,
                instance.universe,
                instance.address,
                encoding,
                pass.output(),
                writes,
            )?;
        }
    }
    Ok(())
}

/// A non-DMX profile publishes its resolved channels for visualization, since nothing encodes them.
fn insert_raw_channel_values(
    visualization: &mut crate::ResolvedValues,
    fixture: &light_fixture::PatchedFixture,
    mode: &light_fixture::FixtureMode,
    output: &ResolvedProfileFixtureOutput,
) {
    for (channel_index, raw) in &output.channels {
        // The resolved channel says which one of the mode it is, so this is an index rather than a
        // scan of every channel per channel.
        let Some(channel) = mode.channels.get(*channel_index as usize) else {
            continue;
        };
        let Some((head_index, head)) = mode
            .heads
            .iter()
            .enumerate()
            .find(|(_, head)| head.id == channel.head_id)
        else {
            continue;
        };
        visualization.insert(
            (
                crate::fixture::profile_head_owner(fixture, head_index, head),
                channel.attribute.clone(),
            ),
            light_core::AttributeValue::RawDmxExact(*raw),
        );
    }
}

fn encode_profile_destination(
    patches: &[light_fixture::SplitPatch],
    legacy_universe: Option<Universe>,
    legacy_address: Option<light_core::DmxAddress>,
    encoding: &light_fixture::FixtureModeEncodingPlan,
    output: &ResolvedProfileFixtureOutput,
    writes: &mut ProjectionWrites<'_>,
) -> Result<(), EngineError> {
    if patches.is_empty() {
        return encode_profile_patch(1, legacy_universe, legacy_address, encoding, output, writes);
    }
    for patch in patches {
        encode_profile_patch(
            patch.split,
            patch.universe,
            patch.address,
            encoding,
            output,
            writes,
        )?;
    }
    Ok(())
}

fn encode_profile_patch(
    split: u16,
    universe: Option<Universe>,
    address: Option<light_core::DmxAddress>,
    encoding: &light_fixture::FixtureModeEncodingPlan,
    output: &ResolvedProfileFixtureOutput,
    writes: &mut ProjectionWrites<'_>,
) -> Result<(), EngineError> {
    let (Some(universe), Some(address)) = (universe, address) else {
        return Ok(());
    };
    let footprint = encoding
        .split_footprint(split)
        .ok_or_else(|| EngineError::Invalid(format!("fixture split {split} has no footprint")))?;
    let frame = writes.universes.entry(universe).or_insert([0; 512]);
    let last_slot = address
        .saturating_sub(1)
        .saturating_add(footprint)
        .min(light_output::DMX_SLOTS as u16);
    writes
        .patched_slots
        .entry(universe)
        .and_modify(|current| *current = (*current).max(last_slot))
        .or_insert(last_slot);
    encode_profile_split(frame, encoding, split, address, output)?;
    Ok(())
}

fn insert_profile_visualization_values(
    values: &mut crate::ResolvedValues,
    output: &ResolvedProfileFixtureOutput,
) {
    for head in &output.heads {
        values.insert(
            (head.owner, light_core::AttributeKey::intensity()),
            light_core::AttributeValue::Normalized(head.intensity),
        );
        if let Some(color) = head.color {
            values.insert(
                (head.owner, light_core::AttributeKey::color()),
                light_core::AttributeValue::ColorXyz(color),
            );
        }
    }
}
