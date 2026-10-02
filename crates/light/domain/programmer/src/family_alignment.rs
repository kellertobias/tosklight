//! Align operates on complete family anchors. Its cumulative input is kept in the component's
//! units; native integers never pass through a floating-point percentage.
use crate::{
    ProgrammerAlignmentError as Error, ProgrammerAlignmentMode as Mode, ProgrammerAlignmentState,
    ProgrammerRegistry,
};
use light_core::programming::*;
use light_core::{AttributeValue, FixtureId, SessionId};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammerAlignmentLane {
    Normal,
    Preload,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FamilyAlignmentInput {
    Scalar(f64),
    Native(i128),
}
impl FamilyAlignmentInput {
    pub fn from_edit(edit: &ComponentEdit) -> Option<(ProgrammingComponent, Self)> {
        match edit {
            ComponentEdit::Scalar {
                component,
                operation: ScalarEdit::Relative(delta),
            } if component.descriptor().align => {
                Some((*component, Self::Scalar(f64::from(*delta))))
            }
            ComponentEdit::Native {
                binding,
                operation: NativeColorEdit::Relative(delta),
            } => Some((
                ProgrammingComponent::NativeColor(*binding),
                Self::Native(i128::from(*delta)),
            )),
            _ => None,
        }
    }
    fn zero(self) -> Self {
        match self {
            Self::Scalar(_) => Self::Scalar(0.0),
            Self::Native(_) => Self::Native(0),
        }
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        match (self, other) {
            (Self::Scalar(a), Self::Scalar(b)) if (a + b).is_finite() => Ok(Self::Scalar(a + b)),
            (Self::Native(a), Self::Native(b)) => a
                .checked_add(b)
                .map(Self::Native)
                .ok_or_else(|| invalid("native Align input overflow")),
            _ => Err(invalid(
                "Align requires finite input in its bound component units",
            )),
        }
    }
    fn subtract(self, other: Self) -> Result<Self, Error> {
        match (self, other) {
            (Self::Scalar(a), Self::Scalar(b)) if (a - b).is_finite() => Ok(Self::Scalar(a - b)),
            (Self::Native(a), Self::Native(b)) => a
                .checked_sub(b)
                .map(Self::Native)
                .ok_or_else(|| invalid("native Align input overflow")),
            _ => Err(invalid("Align input units changed")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerFamilyAlignmentBase {
    pub fixture_id: FixtureId,
    pub rank: usize,
    pub value: AttributeValue,
    pub context: Arc<OwnedFamilyEditContext>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerFamilyAlignmentBinding {
    pub component: ProgrammingComponent,
    pub lane: ProgrammerAlignmentLane,
    pub group_id: Option<String>,
    pub group_seed: Option<Arc<GroupFamilyAssignment>>,
    pub rank_count: usize,
    pub bases: Arc<Vec<ProgrammerFamilyAlignmentBase>>,
    pub input: FamilyAlignmentInput,
    pub anchor_input: FamilyAlignmentInput,
}

/// The initial complete Group template and dormant exceptions are retained even when the first
/// sample is neutral and creates no authored value. Fixtures have no Group seed.
pub struct ProgrammerFamilyAlignmentInitial {
    pub rank_count: usize,
    pub bases: Vec<ProgrammerFamilyAlignmentBase>,
    pub group_seed: Option<GroupFamilyAssignment>,
}

impl From<(usize, Vec<ProgrammerFamilyAlignmentBase>)> for ProgrammerFamilyAlignmentInitial {
    fn from((rank_count, bases): (usize, Vec<ProgrammerFamilyAlignmentBase>)) -> Self {
        Self {
            rank_count,
            bases,
            group_seed: None,
        }
    }
}

pub enum ProgrammerFamilyAlignmentTarget<'a> {
    Fixtures,
    Group {
        id: &'a str,
        current_members: &'a HashSet<FixtureId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerAlignedFamilyValue {
    pub fixture_id: FixtureId,
    pub value: AttributeValue,
    /// A never-moved Target stays unactivated even if other members take over as Angles.
    pub preserves_target: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammerFamilyAlignmentPlan {
    pub expected_revision: u64,
    pub next_state: ProgrammerAlignmentState,
    pub values: Vec<ProgrammerAlignedFamilyValue>,
}

impl ProgrammerRegistry {
    #[allow(clippy::too_many_arguments)]
    pub fn plan_family_alignment_delta(
        &self,
        session: SessionId,
        component: ProgrammingComponent,
        lane: ProgrammerAlignmentLane,
        target: ProgrammerFamilyAlignmentTarget<'_>,
        delta: FamilyAlignmentInput,
        initial: Option<ProgrammerFamilyAlignmentInitial>,
    ) -> Result<ProgrammerFamilyAlignmentPlan, Error> {
        let state = self.alignment(session).ok_or(Error::NotActive)?;
        let (group_id, current_members) = match target {
            ProgrammerFamilyAlignmentTarget::Fixtures => (None, None),
            ProgrammerFamilyAlignmentTarget::Group {
                id,
                current_members,
            } => (Some(id.to_owned()), Some(current_members)),
        };
        if state.binding.is_some() {
            return Err(invalid("Align is bound to a legacy attribute"));
        }
        let mut binding = match &state.family_binding {
            Some(binding) => {
                if binding.component != component
                    || binding.lane != lane
                    || binding.group_id != group_id
                {
                    return Err(invalid(
                        "Align is bound to a different component, lane or Group",
                    ));
                }
                if initial.is_some() {
                    return Err(Error::UnexpectedBases);
                }
                binding.as_ref().clone()
            }
            None => {
                let ProgrammerFamilyAlignmentInitial {
                    rank_count,
                    bases,
                    group_seed,
                } = initial.ok_or(Error::MissingBases)?;
                if group_id.is_some() != group_seed.is_some() {
                    return Err(invalid(
                        "Group Align requires its complete retained template",
                    ));
                }
                if let Some(seed) = &group_seed {
                    seed.validate().map_err(|e| invalid(&e.0))?;
                    if seed.owner != component.owner() {
                        return Err(invalid("Group Align template has a different owner"));
                    }
                }
                validate_bases(&state, component, rank_count, &bases)?;
                ProgrammerFamilyAlignmentBinding {
                    component,
                    lane,
                    group_id,
                    group_seed: group_seed.map(Arc::new),
                    rank_count,
                    bases: Arc::new(bases),
                    input: delta.zero(),
                    anchor_input: delta.zero(),
                }
            }
        };
        // Removed members contribute nothing and cannot be promoted to a new representation.
        // Added members are absent from the binding and inherit the retained Group template.
        if current_members.is_some_and(|members| {
            !binding
                .bases
                .iter()
                .any(|base| members.contains(&base.fixture_id))
        }) {
            return Ok(ProgrammerFamilyAlignmentPlan {
                expected_revision: state.revision,
                next_state: state,
                values: vec![],
            });
        }
        binding.input = binding.input.add(delta)?;
        let movement = binding.input.subtract(binding.anchor_input)?;
        let bases = Arc::make_mut(&mut binding.bases);
        let mut values = Vec::with_capacity(bases.len());
        for base in bases {
            if current_members.is_some_and(|members| !members.contains(&base.fixture_id)) {
                continue;
            }
            let (numerator, denominator) =
                programmer_alignment_fraction(state.mode, base.rank, binding.rank_count)
                    .ok_or_else(|| invalid("invalid frozen Align rank"))?;
            let edit = weighted_edit(component, movement, numerator, denominator)?;
            let value = edit_family(&base.value, &[edit], &base.context.borrowed())
                .map_err(|e| invalid(&e.0))?;
            let original_target = matches!(&base.value,AttributeValue::Position(p) if matches!(p.as_ref(),PositionIntent::Target {..}));
            let preserves_target = original_target
                && base.value == value
                && matches!(
                    component,
                    ProgrammingComponent::Pan | ProgrammingComponent::Tilt
                );
            if original_target
                && matches!(&value,AttributeValue::Position(p) if matches!(p.as_ref(),PositionIntent::Angles {..}))
            {
                // Promote the original anchor, not the moved output. Reversing the accumulated
                // movement back to zero must keep an already-taken-over member in Angles.
                base.value = edit_family(
                    &base.value,
                    &[ComponentEdit::ActivateAngles],
                    &base.context.borrowed(),
                )
                .map_err(|e| invalid(&e.0))?;
            }
            values.push(ProgrammerAlignedFamilyValue {
                fixture_id: base.fixture_id,
                value,
                preserves_target,
            });
        }
        let expected_revision = state.revision;
        let mut next_state = state;
        next_state.family_binding = Some(Arc::new(binding));
        Ok(ProgrammerFamilyAlignmentPlan {
            expected_revision,
            next_state,
            values,
        })
    }

    pub fn commit_family_alignment_plan(
        &self,
        session: SessionId,
        plan: ProgrammerFamilyAlignmentPlan,
    ) -> Result<ProgrammerAlignmentState, Error> {
        self.commit_alignment_state(session, plan.expected_revision, plan.next_state)
    }

    pub fn apply_family_alignment_plan<T>(
        &self,
        session: SessionId,
        plan: ProgrammerFamilyAlignmentPlan,
        mutate: impl FnOnce() -> T,
    ) -> Result<(T, ProgrammerAlignmentState), Error> {
        self.apply_alignment_state(session, plan.expected_revision, plan.next_state, mutate)
    }

    pub fn reanchor_family_alignment(
        &self,
        session: SessionId,
        mode: Mode,
        bases: Vec<ProgrammerFamilyAlignmentBase>,
    ) -> Result<ProgrammerAlignmentState, Error> {
        self.serialized(|| {
            let mut state = self.alignment(session).ok_or(Error::NotActive)?;
            let mut binding = state
                .family_binding
                .as_ref()
                .ok_or_else(|| invalid("Align has no typed binding"))?
                .as_ref()
                .clone();
            validate_bases(&state, binding.component, binding.rank_count, &bases)?;
            if !bases
                .iter()
                .zip(binding.bases.iter())
                .all(|(a, b)| a.fixture_id == b.fixture_id && a.rank == b.rank)
                || bases.len() != binding.bases.len()
            {
                return Err(invalid(
                    "Align reanchoring must preserve frozen membership and ranks",
                ));
            }
            for (next, previous) in bases.iter().zip(binding.bases.iter()) {
                if let (Some(a), Some(b)) =
                    (&next.context.native_model, &previous.context.native_model)
                    && !Arc::ptr_eq(a, b)
                {
                    return Err(invalid(
                        "Align reanchoring must retain its pinned native source model",
                    ));
                }
            }
            binding.bases = Arc::new(bases);
            binding.anchor_input = binding.input;
            let expected = state.revision;
            state.mode = mode;
            state.family_binding = Some(Arc::new(binding));
            self.commit_alignment_state(session, expected, state)
        })
    }
}

fn validate_bases(
    state: &ProgrammerAlignmentState,
    component: ProgrammingComponent,
    rank_count: usize,
    bases: &[ProgrammerFamilyAlignmentBase],
) -> Result<(), Error> {
    if bases.is_empty() || rank_count == 0 {
        return Err(Error::MissingBases);
    }
    let mut seen = HashSet::new();
    let selection = state.fixtures.iter().copied().collect::<HashSet<_>>();
    let mut native_source = None;
    for base in bases {
        if !selection.contains(&base.fixture_id)
            || !seen.insert(base.fixture_id)
            || base.rank >= rank_count
        {
            return Err(Error::BaseFixtureNotInFrozenOrder {
                fixture_id: base.fixture_id,
            });
        }
        base.value
            .validate_programming_scope(ProgrammingValueScope::Fixture)
            .map_err(|e| invalid(&e.0))?;
        base.value
            .validate_programming_address(&component.owner().key())
            .map_err(|e| invalid(&e.0))?;
        if let ProgrammingComponent::NativeColor(binding) = component {
            let model = base
                .context
                .native_model
                .as_ref()
                .ok_or_else(|| invalid("native Align requires a pinned source model"))?;
            if !model
                .descriptor(binding)
                .is_some_and(|d| d.continuous && d.binding == binding)
            {
                return Err(invalid(
                    "native Align requires a verified continuous source function",
                ));
            }
            if native_source
                .as_ref()
                .is_some_and(|source| source != model.source())
            {
                return Err(invalid(
                    "native Align anchors require one verified source layout",
                ));
            }
            native_source = Some(model.source().clone());
        } else if !component.descriptor().align {
            return Err(invalid("this component cannot be aligned"));
        }
    }
    Ok(())
}

/// Exact counterpart to the legacy normalized weights, including odd/even centre pairs.
pub fn programmer_alignment_fraction(mode: Mode, rank: usize, count: usize) -> Option<(u64, u64)> {
    if count == 0 || rank >= count {
        return None;
    }
    if count == 1 {
        return Some((1, 1));
    }
    let (out, span) = if count == 2 {
        (1, 1)
    } else if count.is_multiple_of(2) {
        let span = count / 2 - 1;
        (span - rank.min(count - 1 - rank), span)
    } else {
        let middle = count / 2;
        (rank.abs_diff(middle), middle)
    };
    let (numerator, denominator) = match mode {
        Mode::Left => (rank, count - 1),
        Mode::Right => (count - 1 - rank, count - 1),
        Mode::Out => (out, span),
        Mode::In => (span - out, span),
    };
    Some((numerator as u64, denominator as u64))
}

fn weighted_edit(
    component: ProgrammingComponent,
    input: FamilyAlignmentInput,
    numerator: u64,
    denominator: u64,
) -> Result<ComponentEdit, Error> {
    match (component, input) {
        (ProgrammingComponent::NativeColor(binding), FamilyAlignmentInput::Native(input)) => {
            let product = input
                .checked_mul(i128::from(numerator))
                .ok_or_else(|| invalid("native Align weighted input overflow"))?;
            let divisor = i128::from(denominator);
            let mut rounded = product / divisor;
            if (product % divisor).abs() * 2 >= divisor {
                rounded += product.signum();
            }
            let delta = rounded.clamp(-i128::from(u32::MAX), i128::from(u32::MAX)) as i64;
            Ok(ComponentEdit::Native {
                binding,
                operation: NativeColorEdit::Relative(delta),
            })
        }
        (component, FamilyAlignmentInput::Scalar(input)) if component.descriptor().align => {
            let mut value = input * numerator as f64 / denominator as f64;
            match component.descriptor().domain {
                Some(ScalarDomain::Bounded { .. }) => {
                    value = value.clamp(-f64::from(f32::MAX), f64::from(f32::MAX));
                }
                Some(ScalarDomain::Cyclic { bounds }) => {
                    value = value.rem_euclid(f64::from(bounds.max) - f64::from(bounds.min));
                }
                _ => {}
            }
            if !(value as f32).is_finite() {
                return Err(Error::NonFiniteDelta);
            }
            Ok(ComponentEdit::Scalar {
                component,
                operation: ScalarEdit::Relative(value as f32),
            })
        }
        _ => Err(invalid("Align input does not match the bound component")),
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidFamily(message.to_owned())
}
