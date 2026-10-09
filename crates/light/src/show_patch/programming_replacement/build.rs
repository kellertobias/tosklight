use super::*;
use crate::show_patch::record_index::StoredFixtureRecords;
use crate::show_patch::records::StagedFixture;
use crate::show_patch::{PatchFixtureUpdateAction, PatchFixturesCommand};
use light_engine::ReplacementDestinationPlan;
use light_fixture::{PatchedFixture, PatchedFixtureCompiler, ResolvedFixtureProfileRevision};
use light_show::{PortableShowCandidate, PortableShowDocument};

pub(in crate::show_patch) fn build(
    document: &PortableShowDocument,
    candidate: PortableShowCandidate<'_>,
    command: &PatchFixturesCommand,
    fixtures: &[StagedFixture],
) -> Result<Vec<PatchProgrammingReplacement>, ActionError> {
    let stored = StoredFixtureRecords::load(document)?;
    let mut old_compiler =
        PatchedFixtureCompiler::new(|reference: light_fixture::PatchedFixtureProfileReference| {
            document
                .fixture_profile_revision(reference.profile_id, reference.profile_revision)
                .map(|profile| {
                    ResolvedFixtureProfileRevision::new(
                        profile.id().profile_id(),
                        profile.id().revision(),
                        profile.digest().as_str(),
                        profile.profile().clone(),
                    )
                })
        });
    let mut new_compiler =
        PatchedFixtureCompiler::new(|reference: light_fixture::PatchedFixtureProfileReference| {
            candidate
                .fixture_profile_revision(reference.profile_id, reference.profile_revision)
                .map(|profile| {
                    ResolvedFixtureProfileRevision::new(
                        profile.id().profile_id(),
                        profile.id().revision(),
                        profile.digest().as_str(),
                        profile.profile().clone(),
                    )
                })
        });
    let mut plans = Vec::new();
    for update in &command.fixture_updates {
        let PatchFixtureUpdateAction::ReplaceProfile {
            head_mapping,
            root_programming_mapping,
            ..
        } = &update.action
        else {
            continue;
        };
        let old_record = stored
            .get(update.fixture_id)
            .ok_or_else(|| invalid("replacement source fixture is missing"))?;
        let new_record = fixtures
            .iter()
            .find(|fixture| fixture.patch.fixture_id == update.fixture_id)
            .ok_or_else(|| invalid("replacement candidate fixture is missing"))?;
        let old = old_compiler
            .compile(&old_record.record)
            .map_err(|error| invalid(error.to_string()))?;
        let new = new_compiler
            .compile(&new_record.record)
            .map_err(|error| invalid(error.to_string()))?;
        let (source_profile, old_mode) = context(&old)?;
        let (target_profile, new_mode) = context(&new)?;
        let source_plan = ReplacementDestinationPlan::compile(&[old.clone()])
            .map_err(|error| invalid(error.to_string()))?;
        let target_plan = ReplacementDestinationPlan::compile(&[new.clone()])
            .map_err(|error| invalid(error.to_string()))?;
        let mut attributes: HashSet<AttributeKey> =
            ["intensity", "color", "position", "zoom", "focus", "iris"]
                .into_iter()
                .map(|attribute| AttributeKey(attribute.into()))
                .collect();
        attributes.extend(old_mode.channels.iter().flat_map(|channel| {
            [&channel.attribute, &channel.fixture_attribute]
                .into_iter()
                .cloned()
        }));
        let mut root_attributes = HashSet::new();
        for head in old_mode.heads.iter().filter(|head| head.master_shared) {
            let source = ReplacementProgramProjection {
                source_owner: update.fixture_id,
                source_profile: source_profile.clone(),
                source_head_id: head.id,
                target_profile: source_profile.clone(),
                targets: vec![ReplacementHeadTarget {
                    profile_head_id: head.id,
                    fixture_id: update.fixture_id,
                }],
            };
            root_attributes.extend(
                attributes
                    .iter()
                    .filter(|attribute| !source_plan.destinations(&source, attribute).is_empty())
                    .cloned(),
            );
        }
        let mut root_projections = HashMap::new();
        for mapping in root_programming_mapping {
            let head = old_mode.heads.iter().find(|head| head.id == mapping.source_profile_head_id && head.master_shared)
                .ok_or_else(|| invalid("root programming mapping source must be an authored shared head of the old mode"))?;
            let old_projection = ReplacementProgramProjection {
                source_owner: update.fixture_id,
                source_profile: source_profile.clone(),
                source_head_id: head.id,
                target_profile: source_profile.clone(),
                targets: vec![ReplacementHeadTarget {
                    profile_head_id: head.id,
                    fixture_id: update.fixture_id,
                }],
            };
            if source_plan
                .destinations(&old_projection, &mapping.attribute)
                .is_empty()
            {
                return Err(invalid(
                    "root programming mapping attribute is not physically owned by its old shared head",
                ));
            }
            let targets = mapping
                .target_profile_head_ids
                .iter()
                .map(|id| target(&new, new_mode, *id))
                .collect::<Result<Vec<_>, _>>()?;
            let projection = ReplacementProgramProjection {
                source_owner: update.fixture_id,
                source_profile: source_profile.clone(),
                source_head_id: head.id,
                target_profile: target_profile.clone(),
                targets,
            };
            if target_plan
                .destinations(&projection, &mapping.attribute)
                .len()
                != projection.targets.len()
            {
                return Err(invalid(
                    "replacement destination cannot execute the selected programming attribute",
                ));
            }
            if root_projections
                .insert(mapping.attribute.clone(), projection)
                .is_some()
            {
                return Err(invalid(
                    "choose exactly one source correspondence for each root programming attribute",
                ));
            }
        }
        let head_targets = head_mapping
            .iter()
            .filter_map(|mapping| {
                mapping
                    .target_profile_head_id
                    .map(|id| (mapping.fixture_id, id))
            })
            .map(|(fixture_id, id)| Ok((fixture_id, target(&new, new_mode, id)?)))
            .collect::<Result<HashMap<_, _>, ActionError>>()?;
        let source_head_owners = old_mode
            .heads
            .iter()
            .map(|head| Ok((head.id, target(&old, old_mode, head.id)?.fixture_id)))
            .collect::<Result<HashMap<_, _>, ActionError>>()?;
        plans.push(PatchProgrammingReplacement {
            source_owner: update.fixture_id,
            source_profile,
            target_profile,
            root_attributes,
            root_projections,
            head_targets,
            source_head_owners,
        });
    }
    Ok(plans)
}

fn context(
    fixture: &PatchedFixture,
) -> Result<(ReplacementProfileContext, &light_fixture::FixtureMode), ActionError> {
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or_else(|| invalid("replacement requires an immutable source profile"))?;
    let mode = profile
        .modes
        .iter()
        .find(|mode| Some(mode.id) == fixture.definition.mode_id)
        .ok_or_else(|| invalid("replacement source mode is missing"))?;
    Ok((
        ReplacementProfileContext {
            profile_id: profile.id,
            profile_revision: profile.revision.into(),
            mode_id: mode.id,
        },
        mode,
    ))
}

fn target(
    fixture: &PatchedFixture,
    mode: &light_fixture::FixtureMode,
    id: uuid::Uuid,
) -> Result<ReplacementHeadTarget, ActionError> {
    let (index, head) = mode
        .heads
        .iter()
        .enumerate()
        .find(|(_, head)| head.id == id)
        .ok_or_else(|| invalid("replacement target head is missing"))?;
    let owner = if head.master_shared {
        fixture.fixture_id
    } else {
        fixture
            .logical_heads
            .iter()
            .find(|head| head.profile_head_id == Some(id) && usize::from(head.head_index) == index)
            .map(|head| head.fixture_id)
            .ok_or_else(|| invalid("replacement target has no staged logical owner"))?
    };
    Ok(ReplacementHeadTarget {
        profile_head_id: id,
        fixture_id: owner,
    })
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
