use crate::ProgrammerRegistry;
use light_core::{AttributeKey, FixtureId, SessionId};

const VALUE_PRECISION: f32 = 1_000_000.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammerAlignmentMode {
    Left,
    Right,
    Out,
    In,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignmentBase {
    pub fixture_id: FixtureId,
    pub value: f32,
    pub wraps: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignmentBinding {
    pub attribute: AttributeKey,
    pub bases: Vec<ProgrammerAlignmentBase>,
    /// Accumulated encoder position at the most recent activation or mode switch.
    pub anchor_input_position: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignmentState {
    pub revision: u64,
    pub mode: ProgrammerAlignmentMode,
    /// Selection order frozen when Align was activated.
    pub fixtures: Vec<FixtureId>,
    pub binding: Option<ProgrammerAlignmentBinding>,
    /// Mutually exclusive with the normalized binding. Complete anchors and immutable models
    /// are runtime-only and participate in atomic registry transaction snapshots.
    pub family_binding: Option<std::sync::Arc<crate::ProgrammerFamilyAlignmentBinding>>,
    /// Signed encoder movement accumulated across every accepted sample.
    pub input_position: f32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgrammerAlignmentProjection {
    pub revision: u64,
    pub mode: Option<ProgrammerAlignmentMode>,
    pub binding: Option<ProgrammerAlignmentProjectionBinding>,
    pub fixture_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammerAlignmentProjectionBinding {
    Attribute {
        attribute: AttributeKey,
    },
    Family {
        component: light_core::programming::ProgrammingComponent,
        lane: crate::ProgrammerAlignmentLane,
        group_id: Option<String>,
    },
}

/// Committed Off has its own revision. The shared allocator can advance in a rejected staged
/// command, so its current counter must never be exposed as the live projection revision.
#[derive(Clone, Default)]
pub(crate) struct ProgrammerAlignmentContext {
    pub revision: u64,
    pub active: Option<ProgrammerAlignmentState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignedFixtureValue {
    pub fixture_id: FixtureId,
    pub value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignmentPlan {
    pub expected_revision: u64,
    pub next_state: ProgrammerAlignmentState,
    pub values: Vec<ProgrammerAlignedFixtureValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammerAlignmentError {
    InvalidFamily(String),
    UnknownSession,
    EmptySelection,
    NotActive,
    NonFiniteDelta,
    MissingBases,
    UnexpectedBases,
    BaseFixtureNotInFrozenOrder {
        fixture_id: FixtureId,
    },
    InvalidBaseValue {
        fixture_id: FixtureId,
    },
    DifferentAttribute {
        bound: AttributeKey,
        requested: AttributeKey,
    },
    RevisionConflict {
        expected: u64,
        actual: u64,
    },
}

impl ProgrammerRegistry {
    /// Activate Align without changing Programmer values, history, or value generations.
    ///
    /// The desk-local authoritative selection is frozen in its current order. The first later
    /// relative attribute supplies the bases and binds the state.
    pub fn activate_alignment(
        &self,
        session: SessionId,
        mode: ProgrammerAlignmentMode,
    ) -> Result<ProgrammerAlignmentState, ProgrammerAlignmentError> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains(&session) {
            return Err(ProgrammerAlignmentError::UnknownSession);
        }
        let fixtures = self
            .selection(session)
            .ok_or(ProgrammerAlignmentError::UnknownSession)?
            .selected;
        if fixtures.is_empty() {
            return Err(ProgrammerAlignmentError::EmptySelection);
        }
        let state = ProgrammerAlignmentState {
            revision: self.next_alignment_revision(),
            mode,
            fixtures,
            binding: None,
            family_binding: None,
            input_position: 0.0,
        };
        self.install_alignment(state.clone());
        Ok(state)
    }

    pub fn alignment(&self, session: SessionId) -> Option<ProgrammerAlignmentState> {
        self.sessions
            .read()
            .contains(&session)
            .then(|| self.alignment_context.read().active.clone())?
    }

    pub fn alignment_projection(&self) -> ProgrammerAlignmentProjection {
        self.serialized(|| {
            let context = self.alignment_context.read();
            let Some(state) = &context.active else {
                return ProgrammerAlignmentProjection {
                    revision: context.revision,
                    ..Default::default()
                };
            };
            ProgrammerAlignmentProjection {
                revision: context.revision,
                mode: Some(state.mode),
                fixture_count: state.fixtures.len(),
                binding: state
                    .family_binding
                    .as_ref()
                    .map(|binding| ProgrammerAlignmentProjectionBinding::Family {
                        component: binding.component,
                        lane: binding.lane,
                        group_id: binding.group_id.clone(),
                    })
                    .or_else(|| {
                        state.binding.as_ref().map(|binding| {
                            ProgrammerAlignmentProjectionBinding::Attribute {
                                attribute: binding.attribute.clone(),
                            }
                        })
                    }),
            }
        })
    }

    fn install_alignment(&self, state: ProgrammerAlignmentState) {
        *self.alignment_context.write() = ProgrammerAlignmentContext {
            revision: state.revision,
            active: Some(state),
        };
    }

    /// Re-anchor an active mode from values resolved at the mode-switch instant.
    ///
    /// The frozen fixture order and accumulated encoder position are retained. No Programmer
    /// value, Undo checkpoint, or value generation is changed.
    pub fn reanchor_alignment(
        &self,
        session: SessionId,
        mode: ProgrammerAlignmentMode,
        bases: &[ProgrammerAlignmentBase],
    ) -> Result<ProgrammerAlignmentState, ProgrammerAlignmentError> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let _context = self.command_context(session);
        let current = self
            .alignment_context
            .read()
            .active
            .clone()
            .ok_or(ProgrammerAlignmentError::NotActive)?;
        if current.family_binding.is_some() {
            return Err(ProgrammerAlignmentError::InvalidFamily(
                "typed Align requires complete family reanchoring".into(),
            ));
        }
        let binding = match current.binding.as_ref() {
            Some(binding) => {
                validate_bases(&current.fixtures, bases)?;
                Some(ProgrammerAlignmentBinding {
                    attribute: binding.attribute.clone(),
                    bases: bases.to_vec(),
                    anchor_input_position: current.input_position,
                })
            }
            None if bases.is_empty() => None,
            None => return Err(ProgrammerAlignmentError::UnexpectedBases),
        };
        let state = ProgrammerAlignmentState {
            revision: self.next_alignment_revision(),
            mode,
            fixtures: current.fixtures,
            binding,
            family_binding: None,
            input_position: current.input_position,
        };
        self.install_alignment(state.clone());
        Ok(state)
    }

    /// Plan one relative sample without mutating authoritative Align or Programmer state.
    ///
    /// `bases` is required only for the first sample after activation. The application can
    /// validate and atomically apply `values` before committing `next_state`.
    pub fn plan_alignment_delta(
        &self,
        session: SessionId,
        attribute: AttributeKey,
        delta: f32,
        bases: &[ProgrammerAlignmentBase],
    ) -> Result<ProgrammerAlignmentPlan, ProgrammerAlignmentError> {
        if !delta.is_finite() {
            return Err(ProgrammerAlignmentError::NonFiniteDelta);
        }
        let state = self
            .alignment(session)
            .ok_or(ProgrammerAlignmentError::NotActive)?;
        plan_alignment_delta(state, attribute, delta, bases)
    }

    /// Commit a previously validated plan. The revision check protects callers that do not hold
    /// the application user-and-desk gate; normal application actions hold that gate, so a
    /// conflict after applying values is not expected.
    pub fn commit_alignment_plan(
        &self,
        session: SessionId,
        plan: ProgrammerAlignmentPlan,
    ) -> Result<ProgrammerAlignmentState, ProgrammerAlignmentError> {
        self.commit_alignment_state(session, plan.expected_revision, plan.next_state)
    }

    pub(crate) fn commit_alignment_state(
        &self,
        session: SessionId,
        expected_revision: u64,
        next_state: ProgrammerAlignmentState,
    ) -> Result<ProgrammerAlignmentState, ProgrammerAlignmentError> {
        self.apply_alignment_state(session, expected_revision, next_state, || ())
            .map(|(_, state)| state)
    }

    /// Check the runtime modifier before an infallible, already validated value batch. Holding
    /// the common mutation gate avoids both partial rejection and a full Undo-history clone on
    /// every encoder detent. Callers must not change Align inside the value mutation.
    pub fn apply_alignment_plan<T>(
        &self,
        session: SessionId,
        plan: ProgrammerAlignmentPlan,
        mutate: impl FnOnce() -> T,
    ) -> Result<(T, ProgrammerAlignmentState), ProgrammerAlignmentError> {
        self.apply_alignment_state(session, plan.expected_revision, plan.next_state, mutate)
    }

    pub(crate) fn apply_alignment_state<T>(
        &self,
        session: SessionId,
        expected_revision: u64,
        mut next_state: ProgrammerAlignmentState,
        mutate: impl FnOnce() -> T,
    ) -> Result<(T, ProgrammerAlignmentState), ProgrammerAlignmentError> {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains(&session) {
            return Err(ProgrammerAlignmentError::UnknownSession);
        }
        let _context = self.command_context(session);
        let actual = self
            .alignment_context
            .read()
            .active
            .as_ref()
            .map(|state| state.revision)
            .ok_or(ProgrammerAlignmentError::NotActive)?;
        if actual != expected_revision {
            return Err(ProgrammerAlignmentError::RevisionConflict {
                expected: expected_revision,
                actual,
            });
        }
        let result = mutate();
        next_state.revision = self.next_alignment_revision();
        self.install_alignment(next_state.clone());
        Ok((result, next_state))
    }

    pub fn deactivate_alignment(&self, _session: SessionId) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let mut context = self.alignment_context.write();
        if context.active.take().is_none() {
            return false;
        }
        context.revision = self.next_alignment_revision();
        true
    }

    /// Deactivate only when an already-bound Align context is about to receive another logical
    /// attribute. An unbound context accepts the first attribute and a matching bound context
    /// remains active.
    pub fn deactivate_alignment_if_different(
        &self,
        session: SessionId,
        attribute: &AttributeKey,
    ) -> bool {
        let mutation_gate = self.mutation_gate();
        let _mutation_guard = mutation_gate.lock();
        let _context = self.command_context(session);
        let different = self
            .alignment_context
            .read()
            .active
            .as_ref()
            .and_then(|state| state.binding.as_ref())
            .is_some_and(|binding| binding.attribute != *attribute);
        different && self.deactivate_alignment(session)
    }

    pub(crate) fn next_alignment_revision(&self) -> u64 {
        self.alignment_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1
    }
}

pub fn programmer_alignment_weight(
    mode: ProgrammerAlignmentMode,
    index: usize,
    count: usize,
) -> Option<f32> {
    if count == 0 || index >= count {
        return None;
    }
    if count == 1 {
        return Some(1.0);
    }
    let last = (count - 1) as f32;
    let left = index as f32 / last;
    let out = if count == 2 {
        1.0
    } else if count.is_multiple_of(2) {
        let half = count / 2;
        let distance_from_end = index.min(count - 1 - index) as f32;
        1.0 - distance_from_end / (half - 1) as f32
    } else {
        let middle = count / 2;
        index.abs_diff(middle) as f32 / middle as f32
    };
    Some(match mode {
        ProgrammerAlignmentMode::Left => left,
        ProgrammerAlignmentMode::Right => 1.0 - left,
        ProgrammerAlignmentMode::Out => out,
        ProgrammerAlignmentMode::In => 1.0 - out,
    })
}

pub fn apply_programmer_alignment_delta(base: f32, delta: f32, wraps: bool) -> f32 {
    let value = if wraps {
        (base + delta).rem_euclid(1.0)
    } else {
        (base + delta).clamp(0.0, 1.0)
    };
    (value * VALUE_PRECISION).round() / VALUE_PRECISION
}

fn plan_alignment_delta(
    state: ProgrammerAlignmentState,
    attribute: AttributeKey,
    delta: f32,
    bases: &[ProgrammerAlignmentBase],
) -> Result<ProgrammerAlignmentPlan, ProgrammerAlignmentError> {
    if state.family_binding.is_some() {
        return Err(ProgrammerAlignmentError::InvalidFamily(
            "Align is bound to a typed component".into(),
        ));
    }
    let expected_revision = state.revision;
    let previous_input_position = state.input_position;
    let input_position = previous_input_position + delta;
    if !input_position.is_finite() {
        return Err(ProgrammerAlignmentError::NonFiniteDelta);
    }
    let binding = match state.binding {
        Some(binding) if binding.attribute != attribute => {
            return Err(ProgrammerAlignmentError::DifferentAttribute {
                bound: binding.attribute,
                requested: attribute,
            });
        }
        Some(binding) => {
            if !bases.is_empty() {
                return Err(ProgrammerAlignmentError::UnexpectedBases);
            }
            binding
        }
        None => {
            if bases.is_empty() {
                return Err(ProgrammerAlignmentError::MissingBases);
            }
            validate_bases(&state.fixtures, bases)?;
            ProgrammerAlignmentBinding {
                attribute,
                bases: bases.to_vec(),
                anchor_input_position: previous_input_position,
            }
        }
    };
    let effective_delta = input_position - binding.anchor_input_position;
    let values = binding
        .bases
        .iter()
        .enumerate()
        .map(|(index, base)| ProgrammerAlignedFixtureValue {
            fixture_id: base.fixture_id,
            value: apply_programmer_alignment_delta(
                base.value,
                programmer_alignment_weight(state.mode, index, binding.bases.len())
                    .expect("validated Align bases retain one weight per fixture")
                    * effective_delta,
                base.wraps,
            ),
        })
        .collect();
    Ok(ProgrammerAlignmentPlan {
        expected_revision,
        next_state: ProgrammerAlignmentState {
            revision: expected_revision,
            mode: state.mode,
            fixtures: state.fixtures,
            binding: Some(binding),
            family_binding: None,
            input_position,
        },
        values,
    })
}

fn validate_bases(
    fixtures: &[FixtureId],
    bases: &[ProgrammerAlignmentBase],
) -> Result<(), ProgrammerAlignmentError> {
    if bases.is_empty() {
        return Err(ProgrammerAlignmentError::MissingBases);
    }
    let mut remaining = fixtures.iter();
    for base in bases {
        if remaining
            .by_ref()
            .find(|fixture| **fixture == base.fixture_id)
            .is_none()
        {
            return Err(ProgrammerAlignmentError::BaseFixtureNotInFrozenOrder {
                fixture_id: base.fixture_id,
            });
        }
        if !base.value.is_finite() || !(0.0..=1.0).contains(&base.value) {
            return Err(ProgrammerAlignmentError::InvalidBaseValue {
                fixture_id: base.fixture_id,
            });
        }
    }
    Ok(())
}
