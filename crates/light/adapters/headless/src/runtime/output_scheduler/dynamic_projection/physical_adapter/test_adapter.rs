//! Seam test adapter. It exercises descriptor compilation, adoption, transitions, complete
//! native writes, continuity and passive quality through the real hybrid seam. Its "physics"
//! is an explicit integer model, not Position, Color or optics accuracy.

use super::*;
use light_core::programming::{ColorProgram, PositionIntent, ScalarIntent, TargetReference};
use std::cell::{Cell, RefCell};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SeamFailure {
    /// The family resolver returns malformed-input failure: the whole frame must reject.
    Resolve,
    /// Destination cannot resolve this frame, but retains the underlying family.
    PassiveResolve,
    /// The resolver omits one owned control: completeness validation must reject.
    IncompleteWrites,
    /// No destination model: the scalar owner stays with a passive requirement.
    MissingDestination,
}

pub(in crate::runtime) struct SeamDescriptor {
    pub owner: ProgrammingOwner,
    pub footprint: [NativeControlSlot; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct SeamContinuity {
    pub primary: f32,
    pub frames: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct SeamValues {
    pub primary: f32,
    pub secondary: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct SeamQuality {
    pub clipped: bool,
    /// Continuity observed from this lane's previous accepted frame.
    pub previous: Option<SeamContinuity>,
}

/// Maps two scalar requests to two 16-bit controls: raw = round(value * 100), clamped.
pub(in crate::runtime) struct SeamAdapter {
    pub owners: Vec<ProgrammingOwner>,
    /// Optional shared destination for every target (exercises shared-control conflicts).
    pub shared_destination: Option<FixtureId>,
    pub failure: Cell<Option<SeamFailure>>,
    pub compiles: Cell<usize>,
    pub adoptions: Cell<usize>,
    pub transitions: RefCell<Vec<f32>>,
}

impl SeamAdapter {
    pub fn new(owners: &[ProgrammingOwner]) -> Self {
        Self {
            owners: owners.to_vec(),
            shared_destination: None,
            failure: Cell::new(None),
            compiles: Cell::new(0),
            adoptions: Cell::new(0),
            transitions: RefCell::new(Vec::new()),
        }
    }
}

fn check_frame(frame: HybridFrameContext<'_>) {
    assert!(frame.token.same_capture(&frame.capture.frame_token()));
    assert!(frame.token.matches_geometry(frame.geometry));
}

fn scalar(value: &ScalarIntent) -> Result<f32, TransitionError> {
    match value {
        ScalarIntent::Value(value) => Ok(*value),
        _ => Err(IntentError("seam adapter needs materialized values".into()).into()),
    }
}

/// Explicit seam model: a Target's first two offsets stand in for joints.
fn seam_joints(value: &AttributeValue) -> Result<(f32, f32), TransitionError> {
    match value {
        AttributeValue::Position(intent) => match intent.as_ref() {
            PositionIntent::Angles {
                pan_degrees,
                tilt_degrees,
            } => Ok((scalar(pan_degrees)?, scalar(tilt_degrees)?)),
            PositionIntent::Target { offset_metres, .. } => {
                Ok((scalar(&offset_metres[0])?, scalar(&offset_metres[1])?))
            }
        },
        _ => Err(IntentError("seam Position adapter received another family".into()).into()),
    }
}

fn requested(
    owner: ProgrammingOwner,
    value: &AttributeValue,
) -> Result<SeamValues, TransitionError> {
    match (owner, value) {
        (ProgrammingOwner::Position, _) => {
            let (primary, secondary) = seam_joints(value)?;
            Ok(SeamValues { primary, secondary })
        }
        (ProgrammingOwner::Color, AttributeValue::ColorProgram(program)) => {
            match program.as_ref() {
                ColorProgram::Semantic { intent } => Ok(SeamValues {
                    primary: intent.uv.amount,
                    secondary: 0.,
                }),
                _ => Err(TransitionError::Requires(
                    TransitionRequirement::ColorAppearance,
                )),
            }
        }
        _ => Err(IntentError("seam adapter received an unsupported family".into()).into()),
    }
}

impl PhysicalFamilyAdapter for SeamAdapter {
    type Descriptor = SeamDescriptor;
    type Continuity = SeamContinuity;
    type Requested = SeamValues;
    type Achieved = SeamValues;
    type Quality = SeamQuality;

    fn owns(&self, owner: ProgrammingOwner) -> bool {
        self.owners.contains(&owner)
    }

    fn compile(
        &self,
        _snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<SeamDescriptor>, TransitionError> {
        self.compiles.set(self.compiles.get() + 1);
        if self.failure.get() == Some(SeamFailure::MissingDestination) {
            return Ok(None);
        }
        let owner = self.owners[0];
        let base = match owner {
            ProgrammingOwner::Position => 0,
            ProgrammingOwner::Color => 2,
            ProgrammingOwner::Focus => 4,
            ProgrammingOwner::Zoom => 5,
        };
        let destination = self.shared_destination.unwrap_or(target);
        Ok(Some(SeamDescriptor {
            owner,
            footprint: [0, 1].map(|offset| NativeControlSlot {
                destination,
                channel_index: base + offset,
                split: 0,
            }),
        }))
    }

    fn footprint<'d>(&self, descriptor: &'d SeamDescriptor) -> &'d [NativeControlSlot] {
        &descriptor.footprint
    }

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        check_frame(request.frame);
        if self.failure.get() == Some(SeamFailure::Resolve) {
            return Err(IntentError("injected physical resolver failure".into()).into());
        }
        if self.failure.get() == Some(SeamFailure::PassiveResolve) {
            return Err(TransitionError::Requires(owner_requirement(request.owner)));
        }
        let wanted = requested(request.owner, request.value)?;
        let mut clipped = false;
        let mut encode = |value: f32| {
            let raw = (value * 100.).round();
            clipped |= !(0.0..=65_535.0).contains(&raw);
            raw.clamp(0., 65_535.) as u32
        };
        let raws = [encode(wanted.primary), encode(wanted.secondary)];
        let mut writes: Vec<_> = request
            .descriptor
            .footprint
            .iter()
            .zip(raws)
            .map(|(slot, raw)| NativeControlWrite {
                slot: *slot,
                channel_id: uuid::Uuid::from_u128(u128::from(slot.channel_index) + 1),
                function_id: None,
                raw,
                parked: false,
            })
            .collect();
        if self.failure.get() == Some(SeamFailure::IncompleteWrites) {
            writes.pop();
        }
        let achieved = SeamValues {
            primary: raws[0] as f32 / 100.,
            secondary: raws[1] as f32 / 100.,
        };
        Ok(PhysicalResolution {
            writes,
            requested: wanted,
            achieved,
            quality: SeamQuality {
                clipped,
                previous: request.previous.copied(),
            },
            continuity: SeamContinuity {
                primary: achieved.primary,
                frames: request.previous.map_or(1, |previous| previous.frames + 1),
            },
        })
    }

    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &SeamDescriptor,
        _target: FixtureId,
        original: &AttributeValue,
        _address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        check_frame(frame);
        assert_eq!(descriptor.owner, ProgrammingOwner::Position);
        self.adoptions.set(self.adoptions.get() + 1);
        let AttributeValue::Position(intent) = original else {
            return Err(TransitionError::Requires(owner_requirement(
                descriptor.owner,
            )));
        };
        let PositionIntent::Target {
            reference: TargetReference::Origin,
            offset_metres,
        } = intent.as_ref()
        else {
            return Err(TransitionError::Requires(
                TransitionRequirement::LiveTargetPoints,
            ));
        };
        // Seam model: fixed Pan marker, Tilt from the Target's Y offset.
        Ok(AttributeValue::Position(Arc::new(PositionIntent::angles(
            35.,
            scalar(&offset_metres[1])?,
        ))))
    }

    fn transition(
        &self,
        frame: HybridFrameContext<'_>,
        _descriptor: &SeamDescriptor,
        _target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        check_frame(frame);
        let FamilyExpressionOperation::Transition { progress } = operation else {
            return Err(TransitionError::Requires(requirement));
        };
        self.transitions.borrow_mut().push(progress);
        let (from, to) = (seam_joints(from)?, seam_joints(to)?);
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Ok((
            AttributeValue::Position(Arc::new(PositionIntent::angles(
                mix(from.0, to.0),
                mix(from.1, to.1),
            ))),
            None,
        ))
    }
}
