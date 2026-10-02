//! Observational physical results from the same final native values sent to the encoder.
//! All models are compiled with the patch generation; frame loans retain nested working storage.
use crate::{EngineError, EngineSnapshot, Pooled, Reusable, ValuePool};
use light_core::{FixtureId, spatial::RigidTransform};
use light_fixture::{FixtureProfile, InstalledColorCalibration, forward::*};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalModelSupport {
    Legacy,
    Compiled,
    Unsupported,
}

#[derive(Debug)]
pub struct PhysicalInstanceOutput {
    pub fixture_id: FixtureId,
    /// Root uses fixture UUID; copies use their own stable multipatch UUID.
    pub instance_id: Uuid,
    pub color_support: PhysicalModelSupport,
    pub position_support: PhysicalModelSupport,
    pub optics_support: PhysicalModelSupport,
    pub colors: Vec<ColorForwardResult>,
    pub axes: Vec<AxisForwardCommand>,
    /// Commanded profile-local pose. Mount/Point placement and mechanical settling are separate.
    pub lenses: Vec<LensForwardPose>,
    pub optics: Vec<OpticsForwardResult>,
    pub native_identity: Arc<str>,
    pub native_raw: Box<[u32]>,
    pub complete: bool,
    /// Configuration-time reasons for passive details, never operation-blocking notifications.
    pub color_diagnostic: Option<Arc<str>>,
    pub position_diagnostic: Option<Arc<str>>,
    pub optics_diagnostic: Option<Arc<str>>,
    seen: Box<[bool]>,
    angles: Box<[Option<f64>]>,
    position_workspace: Option<PositionForwardWorkspace>,
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
    color: Result<Option<CompiledColorForward>, Arc<str>>,
    position: Result<Option<CompiledPositionForward>, Arc<str>>,
    optics: Result<CompiledOpticsForward, Arc<str>>,
}
impl Plan {
    fn compile(
        fixture: FixtureId,
        instance: Uuid,
        profile: &FixtureProfile,
        mode: Uuid,
        color: Option<&InstalledColorCalibration>,
        context: Option<&light_fixture::ColorCalibrationContext>,
        position: PositionInstallation<'_>,
        appearance: &light_fixture::InstalledFixtureAppearance,
        patches: &[light_fixture::SplitPatch],
    ) -> Self {
        let mode_data = profile.mode(mode).expect("compiled profile mode exists");
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
            color: CompiledColorForward::compile_with_context(profile, mode, color, context)
                .map(|model| model.map(|m| m.with_installed_appearance(appearance)))
                .map_err(|e| Arc::<str>::from(e.to_string())),
            position: CompiledPositionForward::compile(profile, mode, position)
                .map_err(|e| Arc::<str>::from(e.to_string())),
            optics: CompiledOpticsForward::compile(mode_data)
                .map_err(|e| Arc::<str>::from(e.to_string())),
        }
    }
    fn create_output(&self) -> PhysicalInstanceOutput {
        let support = |present: bool, valid: bool| {
            if !valid {
                PhysicalModelSupport::Unsupported
            } else if present {
                PhysicalModelSupport::Compiled
            } else {
                PhysicalModelSupport::Legacy
            }
        };
        let color = self.color.as_ref().ok().and_then(Option::as_ref);
        let position = self.position.as_ref().ok().and_then(Option::as_ref);
        let optics = self.optics.as_ref().ok();
        let axes = position.map_or_else(Vec::new, CompiledPositionForward::create_commands);
        PhysicalInstanceOutput {
            fixture_id: self.fixture,
            instance_id: self.instance,
            color_support: support(color.is_some(), self.color.is_ok()),
            position_support: support(position.is_some(), self.position.is_ok()),
            optics_support: support(optics.is_some(), self.optics.is_ok()),
            colors: color.map_or_else(Vec::new, CompiledColorForward::create_output),
            lenses: position.map_or_else(Vec::new, CompiledPositionForward::create_output),
            optics: optics.map_or_else(Vec::new, CompiledOpticsForward::create_output),
            angles: vec![None; axes.len()].into_boxed_slice(),
            axes,
            native_identity: self.native_identity.clone(),
            native_raw: vec![0; self.channels].into_boxed_slice(),
            seen: vec![false; self.channels].into_boxed_slice(),
            complete: false,
            color_diagnostic: self.color.as_ref().err().cloned(),
            position_diagnostic: self.position.as_ref().err().cloned(),
            optics_diagnostic: self.optics.as_ref().err().cloned(),
            position_workspace: position.map(CompiledPositionForward::create_workspace),
        }
    }
    fn evaluate(
        &self,
        channels: &[(u32, u32)],
        output: &mut PhysicalInstanceOutput,
    ) -> Result<(), EngineError> {
        output.complete = false;
        output.seen.fill(false);
        for &(index, raw) in channels {
            let index = index as usize;
            if index >= output.native_raw.len() || output.seen[index] {
                return Err(EngineError::Invalid(
                    "physical projection has invalid native channel layout".into(),
                ));
            }
            output.native_raw[index] = raw;
            output.seen[index] = true;
        }
        if output.seen.iter().any(|seen| !*seen) {
            return Err(EngineError::Invalid(
                "physical projection requires a complete native fixture result".into(),
            ));
        }
        let failure = |family| {
            EngineError::Invalid(format!(
                "compiled {family} forward output layout is inconsistent"
            ))
        };
        if let Ok(Some(color)) = &self.color {
            color
                .evaluate(&output.native_raw, &mut output.colors)
                .map_err(|_| failure("Color"))?;
        }
        if let Ok(optics) = &self.optics {
            optics
                .evaluate(&output.native_raw, &mut output.optics)
                .map_err(|_| failure("Focus/Zoom"))?;
        }
        if let Ok(Some(position)) = &self.position {
            position
                .decode_commands(&output.native_raw, &mut output.axes)
                .map_err(|_| failure("Position"))?;
            for (q, c) in output.angles.iter_mut().zip(&output.axes) {
                *q = c.absolute_degrees();
            }
            position
                .evaluate_pose(
                    &output.angles,
                    RigidTransform::IDENTITY,
                    output.position_workspace.as_mut().unwrap(),
                    &mut output.lenses,
                )
                .map_err(|_| failure("Position"))?;
        }
        output.complete = true;
        Ok(())
    }
}

pub(crate) struct PhysicalProjectionIndex {
    layout: Uuid,
    fixtures: HashMap<FixtureId, Box<[usize]>>,
    plans: Vec<Plan>,
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
            fixtures: HashMap::new(),
            plans: Vec::new(),
            pool: Arc::default(),
        }
    }
}
impl PhysicalProjectionIndex {
    pub(crate) fn compile(snapshot: &EngineSnapshot) -> Self {
        let mut result = Self::default();
        for fixture in snapshot.fixtures.iter() {
            let (Some(profile), Some(mode)) = (
                fixture.definition.profile_snapshot.as_deref(),
                fixture.definition.mode_id,
            ) else {
                continue;
            };
            if profile.mode(mode).is_none() {
                continue;
            }
            let first = result.plans.len();
            result.plans.push(Plan::compile(
                fixture.fixture_id,
                fixture.fixture_id.0,
                profile,
                mode,
                fixture.color_calibration.as_ref(),
                fixture.definition.runtime_color_context.as_deref(),
                PositionInstallation {
                    calibration: fixture.position_calibration.as_ref(),
                    invert_pan: fixture.invert_pan,
                    invert_tilt: fixture.invert_tilt,
                    bracket_degrees: f64::from(fixture.bracket_angle),
                },
                &fixture.installed_appearance,
                &fixture.effective_split_patches(),
            ));
            for copy in &fixture.multipatch {
                result.plans.push(Plan::compile(
                    fixture.fixture_id,
                    copy.id,
                    profile,
                    mode,
                    copy.color_calibration.as_ref(),
                    fixture.definition.runtime_color_context.as_deref(),
                    PositionInstallation {
                        calibration: copy.position_calibration.as_ref(),
                        invert_pan: copy.invert_pan,
                        invert_tilt: copy.invert_tilt,
                        bracket_degrees: f64::from(copy.bracket_angle),
                    },
                    &copy.installed_appearance,
                    &copy.effective_split_patches(),
                ));
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
        self.plans[index].evaluate(raw, &mut frame.instances[index])
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
            .as_ref()
    }
}
