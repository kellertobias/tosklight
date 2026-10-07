//! How faithfully each fixture head shows its current Color Intent, for the operator.
//!
//! The legacy report re-resolves each head's current colour target. With the family adapters
//! engaged the headless report instead reads the accepted output frame's published Color
//! results and only borrows [`Engine::color_report_heads`] to name the heads.

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

/// One fixture head a colour report can name: its patched fixture, owning identity and name.
/// Enumerated from the profile alone, without resolving any colour.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorReportHead {
    pub fixture_id: FixtureId,
    /// The selectable identity that owns the head: the fixture itself or one of its logical heads.
    pub owner: FixtureId,
    pub head_id: Uuid,
    pub head_name: String,
}

/// The reportable heads of one fixture, in profile order.
fn fixture_heads(
    generation: &crate::RuntimeGeneration,
    fixture: &light_fixture::PatchedFixture,
) -> Option<Vec<ColorReportHead>> {
    let mode = crate::fixture::profile_mode(fixture)?;
    let projection = generation.profile_projection(fixture.fixture_id)?;
    Some(
        projection
            .heads()
            .iter()
            .map(|head| ColorReportHead {
                fixture_id: fixture.fixture_id,
                owner: head.owner,
                head_id: head.head_id,
                head_name: mode
                    .heads
                    .iter()
                    .find(|candidate| candidate.id == head.head_id)
                    .map(|candidate| candidate.name.clone())
                    .unwrap_or_default(),
            })
            .collect(),
    )
}

impl Engine {
    /// Every colour-reportable head of the given fixtures (all fixtures when `None`), in patch
    /// and profile order, with the runtime generation they were enumerated under. Nothing is
    /// resolved: a caller that reports an accepted output frame pairs these heads with that
    /// frame's own published results and compares the generation with the frame's.
    pub fn color_report_heads(
        &self,
        fixtures: Option<&HashSet<FixtureId>>,
    ) -> (u64, Vec<ColorReportHead>) {
        let generation = self.generation.load();
        let heads = generation
            .snapshot()
            .fixtures
            .iter()
            .filter(|fixture| fixtures.is_none_or(|wanted| wanted.contains(&fixture.fixture_id)))
            .filter_map(|fixture| fixture_heads(&generation, fixture))
            .flatten()
            .collect();
        (generation.identity(), heads)
    }

    /// Legacy report: every head of the given fixtures (all fixtures when `None`), re-resolved
    /// from the current values. A head without a target is reported against white, so the
    /// operator sees what the fixture can do before choosing a colour. The accepted-frame report
    /// (family adapters engaged) does not use this: it reads the published Color results.
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
            let (Some(mode), Some(heads)) = (
                crate::fixture::profile_mode(fixture),
                fixture_heads(&generation, fixture),
            ) else {
                continue;
            };
            for head in heads {
                let target = current(head.owner).or_else(|| current(fixture.fixture_id));
                let resolution = mode.resolve_intent(
                    head.head_id,
                    target.unwrap_or(light_core::color_intent::D65_WHITE),
                );
                report.push(HeadColorReport {
                    fixture_id: fixture.fixture_id,
                    owner: head.owner,
                    head_id: head.head_id,
                    head_name: head.head_name,
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
