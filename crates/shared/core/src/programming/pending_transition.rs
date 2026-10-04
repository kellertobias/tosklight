//! TL-544 G1: an in-flight Position transition that needs the live frame.
//!
//! Angle↔Target crossings and Target→Target fades with a different reference cannot be sampled
//! from the stored values alone: they need live mount, Point and reachable-branch state. The
//! materialized evaluator keeps the source as its frame value (the passive hold every legacy
//! consumer understands) and hands this runtime-only description to the frame instead: the
//! endpoint pair, the progress, and — after an interruption — the frozen transition the new
//! fade starts from. The physical Position adapter evaluates it per destination, so the fade
//! moves through solved joints or world points. It is never persisted, recorded or authored.
use super::*;
use crate::AttributeValue;
use std::sync::Arc;

/// Where a pending transition starts: a materialized value, or an interrupted pending
/// transition frozen at the progress it had when the new fade began.
#[derive(Clone, Debug, PartialEq)]
pub enum PendingTransitionSource {
    Value(AttributeValue),
    Transition(Arc<PendingFamilyTransition>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingFamilyTransition {
    from: PendingTransitionSource,
    to: AttributeValue,
    progress: f32,
}

/// Chains deeper than this collapse their oldest interruption to its held source. Each
/// interruption of a still-running cross-mode fade adds one level; real operation rarely
/// nests more than one or two.
const MAX_DEPTH: usize = 8;

impl PendingFamilyTransition {
    pub fn from(&self) -> &PendingTransitionSource {
        &self.from
    }
    pub fn to(&self) -> &AttributeValue {
        &self.to
    }
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// The value a consumer without a live frame keeps: the innermost materialized source.
    pub fn held(&self) -> &AttributeValue {
        match &self.from {
            PendingTransitionSource::Value(value) => value,
            PendingTransitionSource::Transition(inner) => inner.held(),
        }
    }

    fn depth(&self) -> usize {
        match &self.from {
            PendingTransitionSource::Value(_) => 1,
            PendingTransitionSource::Transition(inner) => 1 + inner.depth(),
        }
    }

    /// Evaluate the chain innermost first. `resolve` samples one pair whose materialized
    /// interpolation reported a live-frame requirement; compatible pairs never reach it.
    pub fn evaluate(
        &self,
        resolve: &mut impl FnMut(
            TransitionRequirement,
            &AttributeValue,
            &AttributeValue,
            f32,
        ) -> Result<AttributeValue, TransitionError>,
    ) -> Result<AttributeValue, TransitionError> {
        let from = match &self.from {
            PendingTransitionSource::Value(value) => value.clone(),
            PendingTransitionSource::Transition(inner) => inner.evaluate(resolve)?,
        };
        match interpolate_programming_value(&from, &self.to, self.progress) {
            Err(TransitionError::Requires(requirement)) => {
                resolve(requirement, &from, &self.to, self.progress)
            }
            sampled => sampled,
        }
    }
}

/// One sampled transition: the materialized frame value and, for a live Position crossing,
/// the pending pair that moves it.
pub type PendingTransitionSample = (AttributeValue, Option<Arc<PendingFamilyTransition>>);

/// Whether a requirement belongs to the Position frame (joints or world points).
pub fn is_live_position_requirement(requirement: TransitionRequirement) -> bool {
    matches!(
        requirement,
        TransitionRequirement::LiveJointAngles | TransitionRequirement::LiveTargetPoints
    )
}

/// Sample `from → to` like [`interpolate_programming_value`], but keep a live Position crossing
/// as a pending pair instead of discarding it. `from_pending` is the still-moving transition
/// the source value was held for, if any; the new fade then starts from its live pose.
///
/// Errors are the evaluator's own: a non-Position requirement (`Requires`) or invalid input.
pub fn sample_programming_transition(
    from: &AttributeValue,
    from_pending: Option<&Arc<PendingFamilyTransition>>,
    to: &AttributeValue,
    progress: f32,
) -> Result<PendingTransitionSample, TransitionError> {
    if progress >= 1.0 {
        return Ok((to.clone(), None));
    }
    let from_pending = from_pending.filter(|pending| pending.held() == from);
    if progress <= 0.0 {
        return Ok((from.clone(), from_pending.cloned()));
    }
    let pending = |source| {
        Arc::new(PendingFamilyTransition {
            from: source,
            to: to.clone(),
            progress,
        })
    };
    if let Some(inner) = from_pending
        && matches!(to, AttributeValue::Position(_))
    {
        let source = if inner.depth() >= MAX_DEPTH {
            PendingTransitionSource::Value(inner.held().clone())
        } else {
            PendingTransitionSource::Transition(Arc::clone(inner))
        };
        return Ok((from.clone(), Some(pending(source))));
    }
    match interpolate_programming_value(from, to, progress) {
        Ok(value) => Ok((value, None)),
        Err(TransitionError::Requires(requirement))
            if is_live_position_requirement(requirement)
                && matches!(
                    (from, to),
                    (AttributeValue::Position(_), AttributeValue::Position(_))
                ) =>
        {
            Ok((
                from.clone(),
                Some(pending(PendingTransitionSource::Value(from.clone()))),
            ))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angles(pan: f32, tilt: f32) -> AttributeValue {
        AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
    }
    fn origin(x: f32) -> AttributeValue {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Origin,
            [x, 0.0, 0.0],
        )))
    }
    fn point(x: f32) -> AttributeValue {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: uuid::Uuid::from_u128(7),
            },
            [x, 0.0, 0.0],
        )))
    }

    #[test]
    fn compatible_pairs_interpolate_without_a_pending_pair() {
        let (value, pending) =
            sample_programming_transition(&angles(0.0, 0.0), None, &angles(720.0, 0.0), 0.5)
                .unwrap();
        assert_eq!(value, angles(360.0, 0.0));
        assert!(pending.is_none());
    }

    #[test]
    fn crossings_hold_the_source_and_carry_the_endpoint_pair_until_completion() {
        for (from, to, requirement) in [
            (
                angles(10.0, 20.0),
                origin(1.0),
                TransitionRequirement::LiveJointAngles,
            ),
            (
                origin(1.0),
                angles(10.0, 20.0),
                TransitionRequirement::LiveJointAngles,
            ),
            (
                origin(1.0),
                point(2.0),
                TransitionRequirement::LiveTargetPoints,
            ),
        ] {
            let (value, pending) = sample_programming_transition(&from, None, &to, 0.25).unwrap();
            assert_eq!(value, from);
            let pending = pending.unwrap();
            assert_eq!(pending.held(), &from);
            assert_eq!(pending.to(), &to);
            assert_eq!(pending.progress(), 0.25);
            let mut seen = None;
            let resolved = pending
                .evaluate(&mut |required, a, b, progress| {
                    seen = Some((required, a.clone(), b.clone(), progress));
                    Ok(b.clone())
                })
                .unwrap();
            assert_eq!(resolved, to);
            assert_eq!(seen, Some((requirement, from.clone(), to.clone(), 0.25)));
            let (done, none) = sample_programming_transition(&from, None, &to, 1.0).unwrap();
            assert_eq!((done, none), (to.clone(), None));
        }
    }

    #[test]
    fn an_interrupted_crossing_starts_the_new_fade_from_its_live_pose() {
        let (held, first) =
            sample_programming_transition(&angles(0.0, 0.0), None, &origin(4.0), 0.5).unwrap();
        // Interrupted toward compatible Angles: still pending because the source moves.
        let (value, second) =
            sample_programming_transition(&held, first.as_ref(), &angles(90.0, 0.0), 0.5)
                .unwrap();
        assert_eq!(value, angles(0.0, 0.0));
        let second = second.unwrap();
        assert_eq!(second.held(), &angles(0.0, 0.0));
        let resolved = second
            .evaluate(&mut |_, from, _, progress| {
                // The inner crossing resolves to a pose halfway at 45°.
                assert_eq!(progress, 0.5);
                assert_eq!(from, &angles(0.0, 0.0));
                Ok(angles(45.0, 0.0))
            })
            .unwrap();
        assert_eq!(resolved, angles(67.5, 0.0));
        // A source that is no longer the pending pair's held value ignores the stale chain.
        let (value, none) =
            sample_programming_transition(&angles(5.0, 0.0), first.as_ref(), &angles(9.0, 0.0), 0.5)
                .unwrap();
        assert_eq!((value, none), (angles(7.0, 0.0), None));
    }

    #[test]
    fn non_position_requirements_remain_the_callers_error() {
        let color = AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(10.0),
            convention: crate::OpeningConvention::Beam,
        }));
        let field = AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(20.0),
            convention: crate::OpeningConvention::Field,
        }));
        assert_eq!(
            sample_programming_transition(&color, None, &field, 0.5),
            Err(TransitionError::Requires(
                TransitionRequirement::ZoomConvention
            ))
        );
    }
}
