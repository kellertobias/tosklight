//! TL-544 G1: a static Cue or Programmer fade that crosses Position representations moves.
//!
//! The materialized evaluators hold the source of an Angle↔Target crossing (or of a Target→Target
//! fade with a different reference) as the static frame value and attach the endpoint pair to
//! the winning static slot. Here, at the one Live/Preload/Blind Position finisher, each physical
//! destination evaluates that pair against its own captured mount, Point and accepted branch:
//! - Angle↔Target blends the destination's solved, unwrapped joints (the Target endpoint is
//!   adopted nearest the accepted pose, so the branch stays continuous);
//! - Target→Target evaluates both world aim points in this capture, interpolates them and
//!   solves once.
//!
//! The per-destination results take the place of the held value exactly as a Position
//! program's destination copies do; the requested intent, the projected static value and every
//! recording path are untouched, so completion lands on the authored destination, which stays
//! live. A destination that cannot evaluate (no reachable solution, shared mechanics with
//! another owner, missing mount) keeps the passive hold for the whole target.
use super::super::super::programming_projection::hybrid::OwnedHybridProjection;
use super::frame_observer::PendingPosition;
use super::*;
use light_dynamics::WholeFamilyExpressionFrameResolver;

/// Per-destination crossing values of every static row whose held value carries a live pending
/// pair, keyed by target. Rows that compose a Position program are left to that program.
pub(super) fn static_crossings(
    adapter: &PositionAdapter,
    frame: HybridFrameContext<'_>,
    rows: &[OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>],
    pending: impl Fn(FixtureId) -> Option<usize>,
    members: &[PendingPosition],
    current: &current_cohort::CapturedCurrentCohorts,
    active_programs: &[FixtureId],
) -> Result<Vec<(FixtureId, Vec<PositionProgramDestination>)>, TransitionError> {
    let mut crossings = Vec::new();
    for row in rows {
        let Some(member) = pending(row.target).map(|index| &members[index]) else {
            continue;
        };
        if member.program.is_some() {
            continue;
        }
        let Some(transition) = frame
            .scalar
            .pending_transition(row.target, ProgrammingOwner::Position.key_ref())
        else {
            continue;
        };
        // Only the exact held static value moves; a composed or replaced row keeps its owner.
        if transition.held() != &row.value {
            continue;
        }
        let mut destinations = Vec::with_capacity(member.descriptor.instances.len());
        for instance in &member.descriptor.instances {
            let bound = destination::PositionDestinationFrame {
                adapter,
                frame,
                descriptor: &member.descriptor,
                target: row.target,
                instance,
                previous: member.previous.as_ref(),
                current,
                active_programs,
            };
            match transition.evaluate(&mut |requirement, from, to, progress| {
                bound.resolve(
                    requirement,
                    from,
                    to,
                    FamilyExpressionOperation::Transition { progress },
                )
            }) {
                Ok(value) => destinations.push(PositionProgramDestination {
                    destination: instance.destination,
                    value,
                    provenance: row.sidecar.provenance.clone(),
                }),
                // No live solution for this copy: the whole target keeps its passive hold.
                Err(TransitionError::Requires(_)) => {
                    destinations.clear();
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        if !destinations.is_empty() {
            crossings.push((row.target, destinations));
        }
    }
    Ok(crossings)
}
