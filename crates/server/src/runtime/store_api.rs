use super::*;

pub(super) async fn store_preload_intent(
    state: &AppState,
    session: &Session,
    show_id: light_core::ShowId,
    input: PreloadStoreInput,
    expected_revision: u64,
) -> Result<StoredPreloadIntent, ApiError> {
    let activation = state.activation_lock.clone().lock_owned().await;
    let entry = state
        .desk
        .lock()
        .show(show_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("show"))?;
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let use_active_preload = programmer.preload_pending.is_empty()
        && programmer.preload_group_pending.is_empty()
        && (!programmer.preload_active.is_empty() || !programmer.preload_group_active.is_empty());
    let fixture_values = if use_active_preload {
        &programmer.preload_active
    } else {
        &programmer.preload_pending
    };
    let group_values = if use_active_preload {
        &programmer.preload_group_active
    } else {
        &programmer.preload_group_pending
    };
    if fixture_values.is_empty() && group_values.is_empty() {
        return Err(ApiError::bad_request(
            "the pending and active preload scenes are empty",
        ));
    }
    let store = ShowStore::open(&entry.path).map_err(ApiError::store)?;
    let prepared = match input.target.as_str() {
        "preset" => prepare_preload_preset(&store, &input, fixture_values, group_values)?,
        "cue" => prepare_preload_cue(&store, &input, fixture_values, group_values)?,
        _ => return Err(ApiError::bad_request("target must be preset or cue")),
    };
    let kind = prepared.kind.as_str().to_owned();
    let object_id = prepared.object_id.clone();
    drop(store);
    let (stored, activation) = store_prepared_preload_target(
        state,
        session,
        &entry,
        activation,
        prepared,
        expected_revision,
    )
    .await?;
    if use_active_preload {
        let _activation = activation;
        #[cfg(test)]
        {
            let pause = Arc::clone(&state.preload_store_release_lifecycle);
            tokio::task::spawn_blocking(move || pause.pause_if_armed())
                .await
                .expect("Preload Store release pause task failed");
        }
        let context = programming_context(session, light_application::ActionSource::Http, None);
        run_programming_interaction(
            state,
            session,
            &context,
            "http_preload_store",
            ProgrammingLockPolicy::AllowLockedReconciliation,
            || {
                state.programmers.release_preload(session.id);
                persist_programmer(state, session)
            },
        )?
        .output?;
    } else {
        drop(activation);
    }
    Ok(StoredPreloadIntent {
        kind,
        object_id,
        revision: stored.revision,
        event_sequence: stored.event_sequence,
        source: if use_active_preload {
            "active_preload"
        } else {
            "pending_preload"
        },
    })
}

pub(super) struct StoredPreloadIntent {
    pub(super) kind: String,
    pub(super) object_id: String,
    pub(super) revision: u64,
    pub(super) event_sequence: Option<u64>,
    pub(super) source: &'static str,
}

struct StoredPreloadTarget {
    revision: u64,
    event_sequence: Option<u64>,
}

async fn store_prepared_preload_target(
    state: &AppState,
    session: &Session,
    entry: &ShowEntry,
    activation: tokio::sync::OwnedMutexGuard<()>,
    prepared: PreparedPreloadTarget,
    expected: u64,
) -> Result<(StoredPreloadTarget, tokio::sync::OwnedMutexGuard<()>), ApiError> {
    let active = state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|active| active.id == entry.id);
    if active {
        let action = active_show_object_action(
            operator_action_context(session, light_application::ActionSource::Http),
            entry.id,
            vec![put_active_show_object(
                prepared.kind,
                prepared.object_id,
                expected,
                prepared.body,
            )],
        );
        let (result, activation) =
            run_active_show_object_action_async(state, activation, action).await?;
        Ok((
            StoredPreloadTarget {
                revision: result
                    .changes
                    .first()
                    .ok_or_else(|| ApiError::internal("Preload Store produced no object change"))?
                    .object_revision,
                event_sequence: Some(result.event_sequence),
            },
            activation,
        ))
    } else {
        backup_show(state, entry)?;
        let revision = ShowStore::open(&entry.path)
            .map_err(ApiError::store)?
            .put_object(
                prepared.kind.as_str(),
                &prepared.object_id,
                &prepared.body,
                expected,
            )
            .map_err(ApiError::store)?;
        Ok((
            StoredPreloadTarget {
                revision,
                event_sequence: None,
            },
            activation,
        ))
    }
}
