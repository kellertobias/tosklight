//! Validated show-owned Playback policies with atomic persistence/runtime installation.
use super::{
    ActiveShowBackupKind, ApiError, AppState, ServerActiveShowPorts, ServerActiveShowUnitOfWork,
};
use light_application::{ActionContext, ActiveShowPorts, ActiveShowUnitOfWork, BackupIdentity};
use light_core::ShowId;
use serde::{Deserialize, Serialize};
pub(super) type PlaybackSoloRegion = light_wire::v2::playback_solo::PlaybackSoloRegion;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct SoloStore {
    pub revision: u64,
    pub regions: Vec<PlaybackSoloRegion>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct PreloadStore {
    pub revision: u64,
    pub scope: light_playback::PlaybackPreloadScope,
}
impl Default for PreloadStore {
    fn default() -> Self {
        Self {
            revision: 0,
            scope: Default::default(),
        }
    }
}
fn read<T: serde::de::DeserializeOwned + Default>(
    state: &AppState,
    show_id: ShowId,
    kind: &str,
) -> Result<T, ApiError> {
    let unit = ServerActiveShowUnitOfWork::begin(state, show_id, ActiveShowBackupKind::ShowObjects)
        .map_err(error)?;
    unit.document().object(kind, "global").map_or_else(
        || Ok(T::default()),
        |object| {
            serde_json::from_value(object.body().clone())
                .map_err(|e| ApiError::bad_request(e.to_string()))
        },
    )
}
pub(super) fn read_playback_solo(state: &AppState, show_id: ShowId) -> Result<SoloStore, ApiError> {
    let mut stored: SoloStore = read(state, show_id, "playback_solo_regions")?;
    stored.regions = validate_playback_solo_regions(stored.regions)?;
    Ok(stored)
}
pub(super) fn read_playback_preload(
    state: &AppState,
    show_id: ShowId,
) -> Result<PreloadStore, ApiError> {
    read(state, show_id, "playback_preload_scope")
}
pub(super) fn validate_playback_solo_regions(
    mut regions: Vec<PlaybackSoloRegion>,
) -> Result<Vec<PlaybackSoloRegion>, ApiError> {
    let mut ids = std::collections::HashSet::new();
    for region in &mut regions {
        region.name = region.name.trim().into();
        let domain: light_playback::PlaybackSoloRegion = serde_json::from_value(
            serde_json::to_value(&region).map_err(|e| ApiError::bad_request(e.to_string()))?,
        )
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
        domain.validate().map_err(ApiError::bad_request)?;
        if !ids.insert(region.id) {
            return Err(ApiError::bad_request("Solo region ids must be unique"));
        }
    }
    Ok(regions)
}
pub(super) fn update_playback_solo(
    state: &AppState,
    context: &ActionContext,
    show_id: ShowId,
    expected_revision: u64,
    regions: &[PlaybackSoloRegion],
    request_id: &str,
) -> Result<(bool, SoloStore), ApiError> {
    let domain: Vec<light_playback::PlaybackSoloRegion> = serde_json::from_value(
        serde_json::to_value(regions).map_err(|e| ApiError::bad_request(e.to_string()))?,
    )
    .map_err(|e| ApiError::bad_request(e.to_string()))?;
    update(
        state,
        context,
        show_id,
        "playback_solo_regions",
        expected_revision,
        request_id,
        |stored: &SoloStore| stored.revision,
        |stored| stored.regions == regions,
        |revision| SoloStore {
            revision,
            regions: regions.to_vec(),
        },
        |snapshot| snapshot.playback_solo_regions = std::sync::Arc::new(domain),
    )
}
pub(super) fn update_playback_preload(
    state: &AppState,
    context: &ActionContext,
    show_id: ShowId,
    expected_revision: u64,
    scope: light_playback::PlaybackPreloadScope,
    request_id: &str,
) -> Result<(bool, PreloadStore), ApiError> {
    scope.validate().map_err(ApiError::bad_request)?;
    let installed = scope.clone();
    update(
        state,
        context,
        show_id,
        "playback_preload_scope",
        expected_revision,
        request_id,
        |stored: &PreloadStore| stored.revision,
        |stored| stored.scope == scope,
        |revision| PreloadStore {
            revision,
            scope: scope.clone(),
        },
        |snapshot| snapshot.playback_preload_scope = installed,
    )
}
fn update<T: serde::de::DeserializeOwned + Serialize + Default>(
    state: &AppState,
    context: &ActionContext,
    show_id: ShowId,
    kind: &str,
    expected: u64,
    request_id: &str,
    revision: impl Fn(&T) -> u64,
    same: impl Fn(&T) -> bool,
    next: impl FnOnce(u64) -> T,
    install: impl FnOnce(&mut light_engine::EngineSnapshot),
) -> Result<(bool, T), ApiError> {
    state.programming.run_active_show_boundary(|| {
        let ports = ServerActiveShowPorts::show_objects(state.clone());
        ports.authorize_mutation(context).map_err(error)?;
        let mut unit = ports.begin_active_show(context, show_id).map_err(error)?;
        let stored: T = unit.document().object(kind, "global").map_or_else(
            || Ok(T::default()),
            |object| {
                serde_json::from_value(object.body().clone())
                    .map_err(|e| ApiError::bad_request(e.to_string()))
            },
        )?;
        if revision(&stored) != expected {
            return Err(ApiError::conflict(
                "Playback policy revision changed; reload before applying",
            ));
        }
        if same(&stored) {
            return Ok((false, stored));
        }
        let next = next(
            expected
                .checked_add(1)
                .ok_or_else(|| ApiError::conflict("Playback policy revision exhausted"))?,
        );
        let mut snapshot = state.output.snapshot().as_ref().clone();
        snapshot.revision = unit.document().revision().value() + 1;
        install(&mut snapshot);
        let prepared = ports.prepare_runtime(snapshot).map_err(error)?;
        let mut transaction = light_show::PortableShowTransaction::new(unit.document().revision());
        transaction.put(
            kind,
            "global",
            serde_json::to_value(&next).map_err(|e| ApiError::bad_request(e.to_string()))?,
        );
        let commit = ports
            .finalize_runtime(context, prepared, || {
                unit.backup(&BackupIdentity {
                    show_id,
                    correlation_id: context.correlation_id,
                    request_id: request_id.into(),
                })?;
                unit.commit(transaction)
            })
            .map_err(error)?;
        state.active_show.update_current(|entry| {
            if entry.id == show_id {
                entry.revision = commit.revision().value();
            }
        });
        Ok((true, next))
    })
}
fn error(error: light_application::ActionError) -> ApiError {
    match error.kind {
        light_application::ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        light_application::ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        light_application::ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        light_application::ActionErrorKind::NotFound => ApiError::not_found(error.message),
        light_application::ActionErrorKind::Conflict => ApiError::conflict(error.message),
        _ => ApiError::internal(error.message),
    }
}

#[cfg(test)]
mod solo_color_tests;
