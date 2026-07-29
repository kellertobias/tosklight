use super::*;

pub(super) fn authoritative_playback_controls(state: &AppState) -> serde_json::Value {
    let now = application_millis(state);
    let speed_groups = state.output.speed_group_snapshots(now);
    let snapshot = state.output.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| {
            serde_json::json!({
                "id":group.id,
                "master":group.master,
                "flash_level":state.output.group_master_flash(&group.id)
            })
        })
        .collect::<Vec<_>>();
    let control = state.output.control_projection();
    let timing = state.installation.configuration();
    serde_json::json!({
        "speed_groups":speed_groups,
        "groups":groups,
        "grand_master":{
            "level":control.grand_master,
            "effective_level":if control.grand_master_flash {1.0} else {control.grand_master},
            "blackout":control.blackout,
            "flash_active":control.grand_master_flash,
            "dynamics_paused":state.output.playback_dynamics().paused
        },
        "programmer_fade_millis":timing.programmer_fade_millis,
        "cue_fade_millis":timing.sequence_master_fade_millis
    })
}

pub(super) type VirtualPlaybackExclusionZone =
    light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionZone;

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VirtualPlaybackExclusionStore {
    pub(super) revision: u64,
    pub(super) zones: Vec<VirtualPlaybackExclusionZone>,
}

#[cfg(test)]
pub(super) fn test_virtual_playback_exclusion_store(
    zones: Vec<VirtualPlaybackExclusionZone>,
) -> VirtualPlaybackExclusionStore {
    VirtualPlaybackExclusionStore { revision: 1, zones }
}

pub(super) const VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_KIND: &str = "virtual_playback_exclusion_zones";
pub(super) const VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_ID: &str = "global";

pub(super) fn read_virtual_playback_exclusions(
    state: &AppState,
    show_id: light_core::ShowId,
) -> Result<VirtualPlaybackExclusionStore, ApiError> {
    use light_application::ActiveShowUnitOfWork;
    let unit = ServerActiveShowUnitOfWork::begin(state, show_id, ActiveShowBackupKind::ShowObjects)
        .map_err(|error| ApiError::internal(error.message))?;
    decode_virtual_playback_exclusions(unit.document())
}

pub(super) fn update_virtual_playback_exclusions(
    state: &AppState,
    show_id: light_core::ShowId,
    expected_revision: u64,
    zones: &[VirtualPlaybackExclusionZone],
    request_id: &str,
) -> Result<(bool, VirtualPlaybackExclusionStore), ApiError> {
    use light_application::{ActiveShowUnitOfWork, BackupIdentity};
    let mut unit =
        ServerActiveShowUnitOfWork::begin(state, show_id, ActiveShowBackupKind::ShowObjects)
            .map_err(|error| ApiError::internal(error.message))?;
    let stored = decode_virtual_playback_exclusions(unit.document())?;
    if stored.revision != expected_revision {
        return Err(ApiError::conflict(format!(
            "Virtual Playback exclusion-zone revision conflict: expected {expected_revision}, actual {}",
            stored.revision
        )));
    }
    if stored.zones.as_slice() == zones {
        return Ok((false, stored));
    }
    unit.backup(&BackupIdentity {
        show_id,
        correlation_id: Uuid::new_v4(),
        request_id: request_id.to_owned(),
    })
    .map_err(|error| ApiError::internal(error.message))?;
    let next = VirtualPlaybackExclusionStore {
        revision: stored.revision.saturating_add(1),
        zones: zones.to_vec(),
    };
    let mut transaction = light_show::PortableShowTransaction::new(unit.document().revision());
    transaction.put(
        VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_KIND,
        VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_ID,
        serde_json::to_value(&next).map_err(|error| ApiError::internal(error.to_string()))?,
    );
    let commit = unit
        .commit(transaction)
        .map_err(|error| ApiError::internal(error.message))?;
    state.active_show.update_current(|entry| {
        if entry.id == show_id {
            entry.revision = commit.revision().value();
        }
    });
    Ok((true, next))
}

fn decode_virtual_playback_exclusions(
    document: &light_show::PortableShowDocument,
) -> Result<VirtualPlaybackExclusionStore, ApiError> {
    document
        .object(
            VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_KIND,
            VIRTUAL_PLAYBACK_EXCLUSION_OBJECT_ID,
        )
        .map_or_else(
            || Ok(VirtualPlaybackExclusionStore::default()),
            |object| {
                serde_json::from_value(object.body().clone()).map_err(|error| {
                    ApiError::bad_request(format!(
                        "incompatible Virtual Playback exclusion-zone schema: {error}"
                    ))
                })
            },
        )
}

#[cfg(test)]
pub(super) fn persist_test_virtual_playback_exclusions(
    state: &AppState,
    show_id: light_core::ShowId,
    store: &VirtualPlaybackExclusionStore,
) {
    let entry = state.active_show.current().unwrap();
    let show_store = match light_show::ShowStore::open(&entry.path) {
        Ok(show_store) => show_store,
        Err(_) => {
            light_show::ShowStore::create(&entry.path, &entry.name)
                .unwrap()
                .0
        }
    };
    show_store.set_identity(show_id, &entry.name, None).unwrap();
    state.active_show.clear_document_cache();
    update_virtual_playback_exclusions(
        state,
        show_id,
        0,
        &store.zones,
        "test-virtual-playback-exclusions",
    )
    .unwrap();
}

pub(super) fn validate_virtual_playback_exclusion_zones(
    input: Vec<VirtualPlaybackExclusionZone>,
) -> Result<Vec<VirtualPlaybackExclusionZone>, ApiError> {
    let mut zone_ids = HashSet::new();
    let mut zones = Vec::with_capacity(input.len());
    for mut zone in input {
        zone.id = zone.id.trim().to_owned();
        zone.name = zone.name.trim().to_owned();
        validate_virtual_playback_exclusion_zone(&mut zone, &mut zone_ids)?;
        zones.push(zone);
    }
    Ok(zones)
}

fn validate_virtual_playback_exclusion_zone(
    zone: &mut VirtualPlaybackExclusionZone,
    zone_ids: &mut HashSet<String>,
) -> Result<(), ApiError> {
    if zone.id.is_empty() || zone.id.len() > 128 || !zone_ids.insert(zone.id.clone()) {
        return Err(ApiError::bad_request(
            "zones[].id must be unique and contain 1-128 characters",
        ));
    }
    if zone.name.is_empty() || zone.name.len() > 80 {
        return Err(ApiError::bad_request(
            "zones[].name must contain 1-80 characters",
        ));
    }
    let mut seen = HashSet::new();
    if let Some(number) = zone.playback_numbers.iter().find(|number| {
        !(light_playback::MIN_VIRTUAL_PLAYBACK..=light_playback::MAX_VIRTUAL_PLAYBACK)
            .contains(*number)
    }) {
        return Err(ApiError::bad_request(format!(
            "zones[].playback_numbers contains {number}; Virtual Playback numbers must be between 1001 and 39100"
        )));
    }
    if zone
        .playback_numbers
        .iter()
        .any(|number| !seen.insert(*number))
    {
        return Err(ApiError::bad_request(
            "zones[].playback_numbers must contain unique Virtual Playback numbers",
        ));
    }
    if zone.playback_numbers.len() < 2 {
        return Err(ApiError::bad_request(
            "zones[].playback_numbers must contain at least two Virtual Playback numbers",
        ));
    }
    Ok(())
}

pub(super) struct VirtualPlaybackExclusionResolver {
    zones: Vec<VirtualPlaybackExclusionZone>,
}

impl VirtualPlaybackExclusionResolver {
    pub(super) fn read(state: &AppState) -> Result<Self, ApiError> {
        let Some(show) = state.active_show.current().clone() else {
            return Ok(Self { zones: Vec::new() });
        };
        let zones = read_virtual_playback_exclusions(state, show.id)?.zones;
        Ok(Self { zones })
    }

    pub(super) fn zone_numbers(&self) -> Vec<Vec<u16>> {
        self.zones
            .iter()
            .map(|zone| zone.playback_numbers.clone())
            .collect()
    }

    pub(super) fn zone_addresses(&self) -> Vec<Vec<light_playback::VirtualPlaybackAddress>> {
        self.zones
            .iter()
            .map(|zone| {
                zone.playback_numbers
                    .iter()
                    .filter_map(|number| {
                        light_playback::VirtualPlaybackAddress::from_number(*number).ok()
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|addresses| addresses.len() >= 2)
            .collect()
    }
}
