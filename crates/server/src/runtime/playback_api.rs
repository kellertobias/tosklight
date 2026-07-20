use super::*;

pub(super) async fn playback_action(
    State(state): State<AppState>,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let id = light_core::CueListId(id);
    let result = playback_service::http_action(
        &state,
        &session,
        PlaybackAddress::CueList(id),
        &action,
        &PoolPlaybackInput::default(),
    )?;
    let payload = playback_service::cue_list_http_payload(result)?;
    emit(
        &state,
        "playback_changed",
        serde_json::json!({"cue_list_id":id,"action":action,"session_id":session.id}),
    );
    Ok(Json(payload))
}
pub(super) async fn playbacks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let snapshot = state.engine.snapshot();
    let active_page = state
        .active_show
        .read()
        .as_ref()
        .and_then(|show| state.desk.lock().desk_page(session.desk.id, show.id).ok())
        .unwrap_or(1);
    let selected_playback = state.active_show.read().as_ref().and_then(|show| {
        state
            .desk
            .lock()
            .selected_playback(session.desk.id, show.id)
            .ok()
            .flatten()
    });
    Ok(Json(serde_json::json!({
        "cue_lists":snapshot.cue_lists,
        "pool":snapshot.playbacks,
        "pages":snapshot.playback_pages,
        "active":state.engine.playback_runtime_status(),
        "desk":session.desk,
        "active_page":active_page,
        "selected_playback":selected_playback,
        "authoritative_controls":authoritative_playback_controls(&state)
    })))
}

pub(super) fn authoritative_playback_controls(state: &AppState) -> serde_json::Value {
    let now = application_millis(state);
    let speed_groups = {
        let controllers = state.speed_groups.lock();
        std::array::from_fn::<_, 5, _>(|index| controllers[index].snapshot(now))
    };
    let snapshot = state.engine.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| {
            serde_json::json!({
                "id":group.id,
                "master":group.master,
                "flash_level":state.engine.group_master_flash(&group.id)
            })
        })
        .collect::<Vec<_>>();
    let control = state.output_control.lock();
    let timing = state.configuration.read();
    serde_json::json!({
        "speed_groups":speed_groups,
        "groups":groups,
        "grand_master":{
            "level":control.options.grand_master,
            "effective_level":if control.grand_master_flash {1.0} else {control.options.grand_master},
            "blackout":control.options.blackout,
            "flash_active":control.grand_master_flash,
            "dynamics_paused":state.engine.playback_dynamics().paused
        },
        "programmer_fade_millis":timing.programmer_fade_millis,
        "cue_fade_millis":timing.sequence_master_fade_millis
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct VirtualPlaybackExclusionZone {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) slots: Vec<u8>,
}

#[derive(Deserialize)]
pub(super) struct VirtualPlaybackExclusionZoneInput {
    pub(super) zones: Vec<VirtualPlaybackExclusionZone>,
}

pub(super) type VirtualPlaybackExclusionSurfaces =
    HashMap<String, Vec<VirtualPlaybackExclusionZone>>;
pub(super) type VirtualPlaybackExclusionStore = HashMap<String, VirtualPlaybackExclusionSurfaces>;

pub(super) fn virtual_playback_exclusion_setting(show_id: light_core::ShowId) -> String {
    format!("virtual_playback_exclusion_zones:{}", show_id.0)
}

pub(super) fn read_virtual_playback_exclusion_store(
    desk: &DeskStore,
    show_id: light_core::ShowId,
) -> VirtualPlaybackExclusionStore {
    desk.setting(&virtual_playback_exclusion_setting(show_id))
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

pub(super) fn validate_virtual_playback_exclusion_zones(
    input: VirtualPlaybackExclusionZoneInput,
) -> Result<Vec<VirtualPlaybackExclusionZone>, ApiError> {
    let mut zone_ids = HashSet::new();
    let mut zones = Vec::with_capacity(input.zones.len());
    for mut zone in input.zones {
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
            "zone ids must be unique and contain 1-128 characters",
        ));
    }
    if zone.name.is_empty() || zone.name.len() > 80 {
        return Err(ApiError::bad_request(
            "zone names must contain 1-80 characters",
        ));
    }
    let mut seen = HashSet::new();
    zone.slots
        .retain(|slot| (1..=144).contains(slot) && seen.insert(*slot));
    if zone.slots.len() < 2 {
        return Err(ApiError::bad_request(
            "an exclusion zone needs at least two cells",
        ));
    }
    Ok(())
}

pub(super) fn write_virtual_playback_exclusion_surface(
    desk: &DeskStore,
    show_id: light_core::ShowId,
    desk_id: Uuid,
    surface_id: &str,
    zones: &[VirtualPlaybackExclusionZone],
) -> Result<(), ApiError> {
    let mut stored = read_virtual_playback_exclusion_store(desk, show_id);
    let surfaces = stored.entry(desk_id.to_string()).or_default();
    if zones.is_empty() {
        surfaces.remove(surface_id);
    } else {
        surfaces.insert(surface_id.to_owned(), zones.to_vec());
    }
    if surfaces.is_empty() {
        stored.remove(&desk_id.to_string());
    }
    desk.set_setting(
        &virtual_playback_exclusion_setting(show_id),
        &serde_json::to_string(&stored).map_err(|error| ApiError::internal(error.to_string()))?,
    )
    .map_err(ApiError::store)
}

pub(super) async fn virtual_playback_exclusion_zones(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    let surfaces = read_virtual_playback_exclusion_store(&state.desk.lock(), show.id)
        .remove(&session.desk.id.to_string())
        .unwrap_or_default();
    Ok(Json(serde_json::json!({
        "show_id": show.id,
        "desk_id": session.desk.id,
        "surfaces": surfaces,
    })))
}

pub(super) async fn put_virtual_playback_exclusion_zones(
    State(state): State<AppState>,
    Path(surface_id): Path<String>,
    headers: HeaderMap,
    Json(input): Json<VirtualPlaybackExclusionZoneInput>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let show = state
        .active_show
        .read()
        .clone()
        .ok_or_else(|| ApiError::bad_request("no show is open"))?;
    if surface_id.trim().is_empty() || surface_id.len() > 128 {
        return Err(ApiError::bad_request(
            "surface id must contain 1-128 characters",
        ));
    }
    let zones = validate_virtual_playback_exclusion_zones(input)?;
    let desk = state.desk.lock();
    write_virtual_playback_exclusion_surface(&desk, show.id, session.desk.id, &surface_id, &zones)?;
    drop(desk);
    emit(
        &state,
        "virtual_playback_exclusion_zones_changed",
        serde_json::json!({"desk_id":session.desk.id,"show_id":show.id,"surface_id":surface_id,"zones":zones}),
    );
    Ok(Json(
        serde_json::json!({"surface_id":surface_id,"zones":zones}),
    ))
}

pub(super) struct VirtualPlaybackExclusionResolver {
    current_page: u8,
    surfaces: VirtualPlaybackExclusionSurfaces,
    pages: HashMap<u8, HashMap<u8, u16>>,
}

impl VirtualPlaybackExclusionResolver {
    pub(super) fn read(state: &AppState, desk_id: Uuid) -> Self {
        let pages = state
            .engine
            .snapshot()
            .playback_pages
            .iter()
            .map(|page| (page.number, page.slots.clone()))
            .collect();
        let Some(show) = state.active_show.read().clone() else {
            return Self {
                current_page: 1,
                surfaces: HashMap::new(),
                pages,
            };
        };
        let desk = state.desk.lock();
        let current_page = desk.desk_page(desk_id, show.id).unwrap_or(1);
        let surfaces = read_virtual_playback_exclusion_store(&desk, show.id)
            .remove(&desk_id.to_string())
            .unwrap_or_default();
        Self {
            current_page,
            surfaces,
            pages,
        }
    }

    pub(super) fn zone_numbers(&self, addressed_page: Option<u8>) -> Vec<Vec<u16>> {
        if !self.applies_to_page(addressed_page) {
            return Vec::new();
        }
        let Some(slots) = self.pages.get(&self.current_page) else {
            return Vec::new();
        };
        self.surfaces
            .values()
            .flatten()
            .map(|zone| zone_numbers(zone, slots))
            .filter(|numbers| numbers.len() >= 2)
            .collect()
    }

    pub(super) fn applies_to_page(&self, addressed_page: Option<u8>) -> bool {
        addressed_page.is_none_or(|page| page == self.current_page)
    }
}

fn zone_numbers(zone: &VirtualPlaybackExclusionZone, slots: &HashMap<u8, u16>) -> Vec<u16> {
    let mut seen = HashSet::new();
    zone.slots
        .iter()
        .filter_map(|slot| slots.get(slot).copied())
        .filter(|number| seen.insert(*number))
        .collect()
}

pub(super) fn virtual_playback_zone_numbers(state: &AppState, desk_id: Uuid) -> Vec<Vec<u16>> {
    VirtualPlaybackExclusionResolver::read(state, desk_id).zone_numbers(None)
}

pub(super) fn virtual_playback_peer_numbers(zones: &[Vec<u16>], activated_number: u16) -> Vec<u16> {
    let mut peers = zones
        .iter()
        .filter(|zone| zone.contains(&activated_number))
        .flat_map(|zone| zone.iter().copied())
        .filter(|number| *number != activated_number)
        .collect::<Vec<_>>();
    peers.sort_unstable();
    peers.dedup();
    peers
}
