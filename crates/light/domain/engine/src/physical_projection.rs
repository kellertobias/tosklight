//! Observational physical results from the same final native values sent to the encoder.
//! All models are compiled with the patch generation; frame loans retain nested working storage.
use crate::{EngineError, EngineSnapshot, Pooled, Reusable, ValuePool};
use light_core::{FixtureId, spatial::RigidTransform};
use light_fixture::{FixtureProfile, InstalledColorCalibration, forward::*};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalModelSupport {
    Legacy,
    Compiled,
    Unsupported,
}

/// One root's or copy's physical results. The final native values and `complete` are captured
/// on the output path; the forward models (Color, Position, Focus/Zoom) are evaluated from them
/// on first read (TL-639), so the output tick no longer pays for results only readers use.
pub struct PhysicalInstanceOutput {
    pub fixture_id: FixtureId,
    /// Root uses fixture UUID; copies use their own stable multipatch UUID.
    pub instance_id: Uuid,
    pub color_support: PhysicalModelSupport,
    pub position_support: PhysicalModelSupport,
    pub optics_support: PhysicalModelSupport,
    pub native_identity: Arc<str>,
    pub native_raw: Box<[u32]>,
    pub complete: bool,
    /// Configuration-time reasons for passive details, never operation-blocking notifications.
    pub color_diagnostic: Option<Arc<str>>,
    pub position_diagnostic: Option<Arc<str>>,
    pub optics_diagnostic: Option<Arc<str>>,
    plan: Arc<Plan>,
    /// TL-553: `native_raw` holds a complete capture for this plan. Survives a pool reset, so an
    /// instance whose final native values are unchanged keeps its forward results.
    captured: bool,
    seen: Box<[bool]>,
    /// The forward results of exactly `native_raw`, evaluated on first read.
    forward: std::sync::OnceLock<PhysicalForwardResults>,
    /// Buffers of superseded results and the Position workspace, reused by the next evaluation
    /// so a frame loan evaluates without allocating.
    scratch: parking_lot::Mutex<ForwardScratch>,
}

#[derive(Default)]
struct ForwardScratch {
    spare: Option<PhysicalForwardResults>,
    angles: Vec<Option<f64>>,
    workspace: Option<PositionForwardWorkspace>,
}

/// Forward-model results of one instance's final native values.
#[derive(Debug, Default)]
pub struct PhysicalForwardResults {
    pub colors: Vec<ColorForwardResult>,
    pub axes: Vec<AxisForwardCommand>,
    /// Commanded profile-local pose. Mount/Point placement and mechanical settling are separate.
    pub lenses: Vec<LensForwardPose>,
    pub optics: Vec<OpticsForwardResult>,
}

impl PhysicalInstanceOutput {
    /// The forward results of `native_raw`, evaluated now if no reader asked before. Empty for
    /// an incomplete capture.
    pub fn forward(&self) -> &PhysicalForwardResults {
        self.forward.get_or_init(|| {
            let mut scratch = self.scratch.lock();
            let mut results = scratch
                .spare
                .take()
                .unwrap_or_else(|| self.plan.empty_forward());
            if self.complete {
                self.plan
                    .evaluate_into(&self.native_raw, &mut results, &mut scratch);
            }
            results
        })
    }

    /// Whether a reader has asked for this capture's forward results yet (tests).
    #[cfg(test)]
    pub(crate) fn forward_evaluated(&self) -> bool {
        self.forward.get().is_some()
    }

    /// The held results no longer describe `native_raw`; keep their buffers for the next read.
    fn supersede_forward(&mut self) {
        if let Some(results) = self.forward.take() {
            self.scratch.get_mut().spare = Some(results);
        }
    }

    pub fn colors(&self) -> &[ColorForwardResult] {
        &self.forward().colors
    }

    pub fn axes(&self) -> &[AxisForwardCommand] {
        &self.forward().axes
    }

    /// Commanded profile-local pose. Mount/Point placement and mechanical settling are separate.
    pub fn lenses(&self) -> &[LensForwardPose] {
        &self.forward().lenses
    }

    pub fn optics(&self) -> &[OpticsForwardResult] {
        &self.forward().optics
    }
}

impl std::fmt::Debug for PhysicalInstanceOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhysicalInstanceOutput")
            .field("fixture_id", &self.fixture_id)
            .field("instance_id", &self.instance_id)
            .field("color_support", &self.color_support)
            .field("position_support", &self.position_support)
            .field("optics_support", &self.optics_support)
            .field("native_identity", &self.native_identity)
            .field("native_raw", &self.native_raw)
            .field("complete", &self.complete)
            .field("forward", &self.forward.get())
            .finish_non_exhaustive()
    }
}

#[derive(Default, Debug)]
pub struct PhysicalForwardFrame {
    layout: Uuid,
    // The cold layout can be reused across runtime generations; the command sample cannot.
    generation: u64,
    pub instances: Vec<PhysicalInstanceOutput>,
}
impl PhysicalForwardFrame {
    pub(crate) fn bind_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    pub(crate) fn belongs_to_generation(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

impl Reusable for PhysicalForwardFrame {
    fn reset(&mut self) {
        self.generation = 0;
        for i in &mut self.instances {
            i.complete = false;
        }
    }
}
#[derive(Debug)]
struct Plan {
    fixture: FixtureId,
    instance: Uuid,
    channels: usize,
    native_identity: Arc<str>,
    color: Result<Option<Arc<CompiledColorForward>>, Arc<str>>,
    position: Result<Option<Arc<CompiledPositionForward>>, Arc<str>>,
    optics: Result<Arc<CompiledOpticsForward>, Arc<str>>,
}

type ColorInputs = (
    Option<InstalledColorCalibration>,
    light_fixture::InstalledFixtureAppearance,
    Option<light_fixture::SharedByIdentity<light_fixture::ColorCalibrationContext>>,
);
type PositionInputs = (
    Option<light_fixture::InstalledPositionCalibration>,
    bool,
    bool,
    u64,
);

/// TL-639: forward models shared by instances of one profile snapshot and mode with identical
/// installation inputs while one physical layout compiles.
#[derive(Default)]
struct ForwardInterner {
    color: light_fixture::CompiledModelInterner<
        ColorInputs,
        Result<Option<Arc<CompiledColorForward>>, Arc<str>>,
    >,
    position: light_fixture::CompiledModelInterner<
        PositionInputs,
        Result<Option<Arc<CompiledPositionForward>>, Arc<str>>,
    >,
    optics: light_fixture::CompiledModelInterner<(), Result<Arc<CompiledOpticsForward>, Arc<str>>>,
}
impl Plan {
    #[allow(clippy::too_many_arguments)]
    fn compile(
        interner: &mut ForwardInterner,
        fixture: FixtureId,
        instance: Uuid,
        profile: &Arc<FixtureProfile>,
        mode: Uuid,
        color: Option<&InstalledColorCalibration>,
        context: Option<&Arc<light_fixture::ColorCalibrationContext>>,
        position: PositionInstallation<'_>,
        appearance: &light_fixture::InstalledFixtureAppearance,
        patches: &[light_fixture::SplitPatch],
    ) -> Self {
        let mode_data = profile.mode(mode).expect("compiled profile mode exists");
        let error = |e: light_fixture::ProfileError| Arc::<str>::from(e.to_string());
        let color_inputs = (
            color.cloned(),
            appearance.clone(),
            context.cloned().map(light_fixture::SharedByIdentity),
        );
        let (color_model, _) = interner
            .color
            .get_or_compile(profile, mode, color_inputs, || {
                CompiledColorForward::compile_with_context(
                    profile,
                    mode,
                    color,
                    context.map(Arc::as_ref),
                )
                .map(|model| model.map(|m| Arc::new(m.with_installed_appearance(appearance))))
                .map_err(error)
            });
        let position_inputs = (
            position.calibration.cloned(),
            position.invert_pan,
            position.invert_tilt,
            position.bracket_degrees.to_bits(),
        );
        let (position_model, _) =
            interner
                .position
                .get_or_compile(profile, mode, position_inputs, || {
                    CompiledPositionForward::compile(profile, mode, position)
                        .map(|model| model.map(Arc::new))
                        .map_err(error)
                });
        let (optics, _) = interner.optics.get_or_compile(profile, mode, (), || {
            CompiledOpticsForward::compile(mode_data)
                .map(Arc::new)
                .map_err(error)
        });
        Self {
            fixture,
            instance,
            channels: mode_data.channels.len(),
            native_identity: native_instance_identity(
                profile,
                mode_data,
                color,
                position,
                appearance,
                &patches
                    .iter()
                    .filter_map(|p| Some((p.split, p.universe?, p.address?)))
                    .collect::<Vec<_>>(),
            )
            .into(),
            color: color_model,
            position: position_model,
            optics,
        }
    }
    fn create_output(self: &Arc<Self>) -> PhysicalInstanceOutput {
        let support = |present: bool, valid: bool| {
            if !valid {
                PhysicalModelSupport::Unsupported
            } else if present {
                PhysicalModelSupport::Compiled
            } else {
                PhysicalModelSupport::Legacy
            }
        };
        let color = self.color.as_ref().ok().and_then(Option::as_deref);
        let position = self.position.as_ref().ok().and_then(Option::as_deref);
        let optics = self.optics.as_deref().ok();
        PhysicalInstanceOutput {
            fixture_id: self.fixture,
            instance_id: self.instance,
            color_support: support(color.is_some(), self.color.is_ok()),
            position_support: support(position.is_some(), self.position.is_ok()),
            optics_support: support(optics.is_some(), self.optics.is_ok()),
            native_identity: self.native_identity.clone(),
            native_raw: vec![0; self.channels].into_boxed_slice(),
            seen: vec![false; self.channels].into_boxed_slice(),
            complete: false,
            captured: false,
            color_diagnostic: self.color.as_ref().err().cloned(),
            position_diagnostic: self.position.as_ref().err().cloned(),
            optics_diagnostic: self.optics.as_ref().err().cloned(),
            plan: Arc::clone(self),
            forward: std::sync::OnceLock::new(),
            scratch: Default::default(),
        }
    }

    /// The output layout of every compiled model, before any evaluation.
    fn empty_forward(&self) -> PhysicalForwardResults {
        let color = self.color.as_ref().ok().and_then(Option::as_deref);
        let position = self.position.as_ref().ok().and_then(Option::as_deref);
        let optics = self.optics.as_deref().ok();
        PhysicalForwardResults {
            colors: color.map_or_else(Vec::new, CompiledColorForward::create_output),
            axes: position.map_or_else(Vec::new, CompiledPositionForward::create_commands),
            lenses: position.map_or_else(Vec::new, CompiledPositionForward::create_output),
            optics: optics.map_or_else(Vec::new, CompiledOpticsForward::create_output),
        }
    }

    /// Evaluates every compiled forward model on a complete native capture, into buffers this
    /// plan created, so their layouts always match. A failing model (a compiler defect) keeps
    /// its previous values.
    fn evaluate_into(
        &self,
        native_raw: &[u32],
        results: &mut PhysicalForwardResults,
        scratch: &mut ForwardScratch,
    ) {
        if let Ok(Some(color)) = &self.color {
            let evaluated = color.evaluate(native_raw, &mut results.colors);
            debug_assert!(evaluated.is_ok(), "compiled Color forward layout");
        }
        if let Ok(optics) = &self.optics {
            let evaluated = optics.evaluate(native_raw, &mut results.optics);
            debug_assert!(evaluated.is_ok(), "compiled Focus/Zoom forward layout");
        }
        if let Ok(Some(position)) = &self.position {
            let decoded = position.decode_commands(native_raw, &mut results.axes);
            debug_assert!(decoded.is_ok(), "compiled Position command layout");
            scratch.angles.clear();
            scratch.angles.extend(
                results
                    .axes
                    .iter()
                    .map(AxisForwardCommand::absolute_degrees),
            );
            let workspace = scratch
                .workspace
                .get_or_insert_with(|| position.create_workspace());
            let posed = position.evaluate_pose(
                &scratch.angles,
                RigidTransform::IDENTITY,
                workspace,
                &mut results.lenses,
            );
            debug_assert!(posed.is_ok(), "compiled Position pose layout");
        }
    }

    /// Captures the final native values of one instance. The forward results stay those of the
    /// previous capture when every value is unchanged, and are otherwise evaluated on first read.
    fn capture(
        &self,
        channels: &[(u32, u32)],
        output: &mut PhysicalInstanceOutput,
    ) -> Result<(), EngineError> {
        output.complete = false;
        output.seen.fill(false);
        let mut unchanged = output.captured;
        for &(index, raw) in channels {
            let index = index as usize;
            if index >= output.native_raw.len() || output.seen[index] {
                output.captured = false;
                output.supersede_forward();
                return Err(EngineError::Invalid(
                    "physical projection has invalid native channel layout".into(),
                ));
            }
            unchanged &= output.native_raw[index] == raw;
            output.native_raw[index] = raw;
            output.seen[index] = true;
        }
        if output.seen.iter().any(|seen| !*seen) {
            output.captured = false;
            output.supersede_forward();
            return Err(EngineError::Invalid(
                "physical projection requires a complete native fixture result".into(),
            ));
        }
        // The forward models are pure functions of the compiled plan and the native values: the
        // results already held for these exact values are the results of evaluating them again.
        if !unchanged {
            output.supersede_forward();
        }
        output.captured = true;
        output.complete = true;
        Ok(())
    }
}

pub(crate) struct PhysicalProjectionIndex {
    layout: Uuid,
    /// Looked up once per fixture and frame, so hashed for speed (TL-553).
    fixtures: FxHashMap<FixtureId, Box<[usize]>>,
    plans: Vec<Arc<Plan>>,
    pool: Arc<ValuePool<PhysicalForwardFrame>>,
}
impl std::fmt::Debug for PhysicalProjectionIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhysicalProjectionIndex")
            .field("instances", &self.plans.len())
            .finish()
    }
}
impl Default for PhysicalProjectionIndex {
    fn default() -> Self {
        Self {
            layout: Uuid::new_v4(),
            fixtures: FxHashMap::default(),
            plans: Vec::new(),
            pool: Arc::default(),
        }
    }
}
impl PhysicalProjectionIndex {
    pub(crate) fn compile(snapshot: &EngineSnapshot) -> Self {
        let mut result = Self::default();
        let mut interner = ForwardInterner::default();
        for fixture in snapshot.fixtures.iter() {
            let (Some(profile), Some(mode)) = (
                fixture.definition.profile_snapshot.as_ref(),
                fixture.definition.mode_id,
            ) else {
                continue;
            };
            if profile.mode(mode).is_none() {
                continue;
            }
            let first = result.plans.len();
            result.plans.push(Arc::new(Plan::compile(
                &mut interner,
                fixture.fixture_id,
                fixture.fixture_id.0,
                profile,
                mode,
                fixture.color_calibration.as_ref(),
                fixture.definition.runtime_color_context.as_ref(),
                PositionInstallation {
                    calibration: fixture.position_calibration.as_ref(),
                    invert_pan: fixture.invert_pan,
                    invert_tilt: fixture.invert_tilt,
                    bracket_degrees: f64::from(fixture.bracket_angle),
                },
                &fixture.installed_appearance,
                &fixture.effective_split_patches(),
            )));
            for copy in &fixture.multipatch {
                result.plans.push(Arc::new(Plan::compile(
                    &mut interner,
                    fixture.fixture_id,
                    copy.id,
                    profile,
                    mode,
                    copy.color_calibration.as_ref(),
                    fixture.definition.runtime_color_context.as_ref(),
                    PositionInstallation {
                        calibration: copy.position_calibration.as_ref(),
                        invert_pan: copy.invert_pan,
                        invert_tilt: copy.invert_tilt,
                        bracket_degrees: f64::from(copy.bracket_angle),
                    },
                    &copy.installed_appearance,
                    &copy.effective_split_patches(),
                )));
            }
            result
                .fixtures
                .insert(fixture.fixture_id, (first..result.plans.len()).collect());
        }
        result
    }
    /// Exact cold profile/copy/calibration layout which produced this observational frame.
    pub(crate) fn layout_matches(&self, frame: &PhysicalForwardFrame) -> bool {
        frame.layout == self.layout
    }

    pub(crate) fn take_frame(&self) -> Pooled<PhysicalForwardFrame> {
        let mut frame = self.pool.take();
        if frame.layout != self.layout {
            frame.instances = self.plans.iter().map(Plan::create_output).collect();
            frame.layout = self.layout;
        }
        frame
    }
    pub(crate) fn evaluate(
        &self,
        fixture: FixtureId,
        instance: usize,
        raw: &[(u32, u32)],
        frame: &mut PhysicalForwardFrame,
    ) -> Result<(), EngineError> {
        let index = self
            .fixtures
            .get(&fixture)
            .and_then(|v| v.get(instance))
            .copied()
            .ok_or_else(|| {
                EngineError::Invalid("physical projection instance is missing".into())
            })?;
        if frame.layout != self.layout {
            return Err(EngineError::Invalid(
                "physical projection generation mismatch".into(),
            ));
        }
        self.plans[index].capture(raw, &mut frame.instances[index])
    }

    /// The cold Color forward model compiled for one physical root or copy of `fixture`, if
    /// that instance has a valid one. Shared with native family footprints (TL-548 C2).
    pub(crate) fn color_forward(
        &self,
        fixture: FixtureId,
        instance: Uuid,
    ) -> Option<&CompiledColorForward> {
        self.fixtures
            .get(&fixture)?
            .iter()
            .map(|&index| &self.plans[index])
            .find(|plan| plan.instance == instance)?
            .color
            .as_ref()
            .ok()?
            .as_deref()
    }
}
