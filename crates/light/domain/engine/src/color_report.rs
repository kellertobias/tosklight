//! How faithfully each fixture head shows its current Color Intent, for the operator.
//!
//! Resolution is deterministic, so the report re-resolves each head's current colour target
//! rather than keeping per-frame bookkeeping in the render path: it names exactly the values the
//! output is driving.

use crate::{Engine, EngineError};
use light_core::{AttributeKey, AttributeValue, ColorResolutionQuality, FixtureId, Xyz};
use light_fixture::ColorIntentEngine;
use std::collections::HashSet;
use uuid::Uuid;

/// One head's Color Intent result.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadColorReport {
    /// The patched fixture the head belongs to.
    pub fixture_id: FixtureId,
    /// The selectable identity that owns the head: the fixture itself or one of its logical heads.
    pub owner: FixtureId,
    pub head_id: Uuid,
    pub head_name: String,
    /// The head's current colour target, if any source programs one.
    pub target: Option<Xyz>,
    pub quality: ColorResolutionQuality,
    pub engine: Option<ColorIntentEngine>,
    pub delta_uv: Option<f32>,
    pub calibration_revision: Option<u32>,
}

impl Engine {
    /// Report every head of the given fixtures (all fixtures when `None`). A head without a target
    /// is reported against white, so the operator sees what the fixture can do before choosing a
    /// colour.
    pub fn color_intent_report(
        &self,
        fixtures: Option<&HashSet<FixtureId>>,
    ) -> Result<Vec<HeadColorReport>, EngineError> {
        let generation = self.generation.load();
        let snapshot = generation.snapshot();
        let values = self.resolved_values();
        let color = AttributeKey::color();
        let current = |owner: FixtureId| match values.get(&(owner, color.clone())) {
            Some(AttributeValue::ColorXyz(target)) => Some(*target),
            _ => None,
        };
        let mut report = Vec::new();
        for fixture in snapshot.fixtures.iter() {
            if fixtures.is_some_and(|wanted| !wanted.contains(&fixture.fixture_id)) {
                continue;
            }
            let Some(mode) = crate::fixture::profile_mode(fixture) else {
                continue;
            };
            let Some(projection) = generation.profile_projection(fixture.fixture_id) else {
                continue;
            };
            for head in projection.heads() {
                let target = current(head.owner).or_else(|| current(fixture.fixture_id));
                let resolution = mode.resolve_intent(
                    head.head_id,
                    target.unwrap_or(light_core::color_intent::D65_WHITE),
                );
                report.push(HeadColorReport {
                    fixture_id: fixture.fixture_id,
                    owner: head.owner,
                    head_id: head.head_id,
                    head_name: mode
                        .heads
                        .iter()
                        .find(|candidate| candidate.id == head.head_id)
                        .map(|candidate| candidate.name.clone())
                        .unwrap_or_default(),
                    target,
                    quality: resolution.quality,
                    engine: resolution.engine,
                    delta_uv: resolution.delta_uv,
                    calibration_revision: resolution.calibration_revision,
                });
            }
        }
        Ok(report)
    }
}
