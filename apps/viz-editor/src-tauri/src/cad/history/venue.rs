//! Whole venue edits form one history step, including their dimensions and linked openings.
use super::*;
use crate::contract::OutcomeDto;

#[derive(Clone)]
pub(super) struct VenueRecord {
    before: Vec<serde_json::Value>,
    after: Vec<serde_json::Value>,
    added: Vec<Uuid>,
}

#[tauri::command]
pub fn cad_patch_venue(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    cad: tauri::State<'_, CadState>,
    expected_scene_revision: u64,
    fixtures: Vec<FixtureDto>,
) -> Result<OutcomeDto, String> {
    session.gesture(|| {
        check_revision(&session, expected_scene_revision)?;
        if fixtures.is_empty()
            || fixtures
                .iter()
                .any(|f| f.scenery_options.architecture.is_none())
        {
            return Err("Choose venue elements to edit".into());
        }
        let ids: BTreeSet<_> = fixtures.iter().map(|f| f.fixture_id).collect();
        if ids.len() != fixtures.len() {
            return Err("Edit each venue element once".into());
        }
        let snapshot = session.with(|d| d.patch_snapshot().map_err(|e| e.to_string()))?;
        let existing: BTreeSet<_> = snapshot
            .fixtures
            .iter()
            .map(|f| f.patch.fixture_id.0)
            .collect();
        let locked: BTreeSet<_> = existing
            .difference(&selectable_ids(&session)?)
            .copied()
            .collect();
        if !ids.is_disjoint(&locked) {
            return Err("One or more venue elements belong to a locked layer".into());
        }
        let before = snapshot
            .fixtures
            .into_iter()
            .filter(|f| ids.contains(&f.patch.fixture_id.0))
            .map(|f| serde_json::to_value(FixtureDto::from(f)).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let after = fixtures
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let record = VenueRecord {
            before,
            after,
            added: ids.difference(&existing).copied().collect(),
        };
        let outcome = apply_patch_mutation(
            &app,
            &session,
            &cad,
            None,
            MutationDto {
                request_id: Uuid::new_v4().to_string(),
                fixtures,
                remove_fixture_ids: Vec::new(),
                placements: Vec::new(),
            },
        )?;
        if outcome.changed {
            let mut history = cad.history.lock();
            history.undo.push(Step::Venue(record));
            history.redo.clear();
        }
        Ok(outcome)
    })
}

pub(super) fn restore(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    expected: u64,
    record: &VenueRecord,
    forward: bool,
) -> Result<TransformOutcome, String> {
    check_revision(session, expected)?;
    let values = if forward {
        &record.after
    } else {
        &record.before
    };
    let fixtures = values
        .iter()
        .map(|v| serde_json::from_value(v.clone()).map_err(|e| e.to_string()))
        .collect::<Result<Vec<FixtureDto>, _>>()?;
    apply_patch_mutation(
        app,
        session,
        cad,
        None,
        MutationDto {
            request_id: Uuid::new_v4().to_string(),
            fixtures,
            remove_fixture_ids: if forward {
                Vec::new()
            } else {
                record.added.clone()
            },
            placements: Vec::new(),
        },
    )?;
    Ok(TransformOutcome {
        scene_revision: current_revision(session)?,
        transforms: Vec::new(),
        attachments: attachments(session)?,
    })
}
