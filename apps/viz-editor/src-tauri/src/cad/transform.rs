//! The CAD move command's body, run as one operator gesture by [`super::cad_transform`].

use super::*;

pub(super) fn transform(
    app: &tauri::AppHandle,
    session: &Session,
    cad: &CadState,
    intent: TransformIntent,
) -> Result<TransformOutcome, String> {
    if intent.entity_ids.is_empty() {
        return Err("Select at least one fixture or venue object to move".to_owned());
    }
    let ordered_ids = intent.entity_ids;
    let ids: BTreeSet<Uuid> = ordered_ids.iter().copied().collect();
    if ids.len() != ordered_ids.len() {
        return Err("A CAD transform cannot contain the same entity more than once".to_owned());
    }
    if !ids.is_subset(&selectable_ids(session)?) {
        return Err("One or more selected CAD entities belong to a locked layer".to_owned());
    }
    let before = session.with(|document| {
        let patch = document
            .patch_snapshot()
            .map_err(|error| error.to_string())?;
        if patch.patch_revision.value() != intent.expected_scene_revision {
            return Err(format!(
                "The rig changed at revision {}; refresh before committing revision {}",
                patch.patch_revision.value(),
                intent.expected_scene_revision
            ));
        }
        selected_transforms(&patch, &ids)
    })?;
    if before.len() != ids.len() {
        return Err("One or more selected CAD entities no longer exist".to_owned());
    }
    let before_attachments = attachments(session)?
        .into_iter()
        .filter(|attachment| ids.contains(&attachment.fixture_id))
        .collect::<Vec<_>>();
    // The CAD view snaps the move before it sends it (see `snapping.ts`), so the delta already lands
    // a clamp on its pipe; moving it again here would pull it off the pipe it was snapped onto.
    let after = moved_transforms(
        &before,
        &ordered_ids,
        intent.delta_millimetres,
        intent.spread,
    );
    let revision = apply_transforms(session, intent.expected_scene_revision, &after)?;
    let changed_attachments = if intent.snap_to_mounts {
        snap_attachments(session, &after)?
    } else {
        clear_attachments(session, &after)?;
        Vec::new()
    };
    let after_attachments = attachments(session)?
        .into_iter()
        .filter(|attachment| ids.contains(&attachment.fixture_id))
        .collect::<Vec<_>>();
    cad.history.lock().record_move(TransformRecord {
        before,
        after: after.clone(),
        before_attachments,
        after_attachments,
    });
    emit_scene_delta(app, session, cad, revision, Vec::new())?;
    Ok(TransformOutcome {
        scene_revision: revision,
        transforms: after,
        attachments: changed_attachments,
    })
}
