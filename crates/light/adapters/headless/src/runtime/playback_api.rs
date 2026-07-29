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
pub(super) type VirtualPlaybackExclusionSurface =
    light_wire::v2::virtual_playback_zones::VirtualPlaybackExclusionSurface;
pub(super) type VirtualPlaybackSurfacePageMode =
    light_wire::v2::virtual_playback_zones::VirtualPlaybackSurfacePageMode;

pub(super) type VirtualPlaybackExclusionSurfaces =
    HashMap<String, VirtualPlaybackExclusionSurface>;
pub(super) type VirtualPlaybackExclusionStore = HashMap<String, VirtualPlaybackExclusionSurfaces>;

#[cfg(test)]
pub(super) fn test_virtual_playback_exclusion_surface(
    zones: Vec<VirtualPlaybackExclusionZone>,
) -> VirtualPlaybackExclusionSurface {
    VirtualPlaybackExclusionSurface {
        revision: 1,
        page_mode: VirtualPlaybackSurfacePageMode::FollowMain,
        zones,
    }
}

pub(super) fn virtual_playback_exclusion_setting(show_id: light_core::ShowId) -> String {
    format!("virtual_playback_exclusion_zones:{}", show_id.0)
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
    if let Some(slot) = zone
        .slots
        .iter()
        .find(|slot| !(1..=8_998).contains(*slot))
    {
        return Err(ApiError::bad_request(format!(
            "zones[].slots contains {slot}; cells must be between 1 and 8998"
        )));
    }
    if zone.slots.iter().any(|slot| !seen.insert(*slot)) {
        return Err(ApiError::bad_request(
            "zones[].slots must contain unique cells",
        ));
    }
    if zone.slots.len() < 2 {
        return Err(ApiError::bad_request(
            "zones[].slots must contain at least two cells",
        ));
    }
    Ok(())
}

pub(super) struct VirtualPlaybackExclusionResolver {
    current_page: u8,
    surfaces: VirtualPlaybackExclusionSurfaces,
}

impl VirtualPlaybackExclusionResolver {
    pub(super) fn read(state: &AppState, desk_id: Uuid) -> Self {
        let Some(show) = state.active_show.current().clone() else {
            return Self {
                current_page: 1,
                surfaces: HashMap::new(),
            };
        };
        let current_page = state.installation.desk_page(desk_id, show.id).unwrap_or(1);
        let surfaces = state
            .installation
            .virtual_playback_exclusions(show.id)
            .unwrap_or_default()
            .remove(&desk_id.to_string())
            .unwrap_or_default();
        Self {
            current_page,
            surfaces,
        }
    }

    pub(super) fn zone_numbers(&self, addressed_page: Option<u8>) -> Vec<Vec<u16>> {
        self.surfaces
            .values()
            .filter(|surface| {
                addressed_page.is_none_or(|page| self.surface_page(surface) == page)
            })
            .flat_map(|surface| &surface.zones)
            .map(zone_numbers)
            .filter(|numbers| numbers.len() >= 2)
            .collect()
    }

    pub(super) fn zone_addresses(
        &self,
        addressed_page: u8,
    ) -> Vec<Vec<light_playback::VirtualPlaybackAddress>> {
        self.surfaces
            .values()
            .filter(|surface| self.surface_page(surface) == addressed_page)
            .flat_map(|surface| &surface.zones)
            .map(|zone| {
                zone.slots
                    .iter()
                    .filter_map(|cell| {
                        light_playback::VirtualPlaybackAddress::new(
                            addressed_page,
                            1_000 + cell,
                        )
                        .ok()
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|addresses| addresses.len() >= 2)
            .collect()
    }

    pub(super) fn applies_to_page(&self, addressed_page: Option<u8>) -> bool {
        addressed_page.is_none_or(|page| {
            self.surfaces
                .values()
                .any(|surface| self.surface_page(surface) == page)
        })
    }

    fn surface_page(&self, surface: &VirtualPlaybackExclusionSurface) -> u8 {
        match surface.page_mode {
            VirtualPlaybackSurfacePageMode::FollowMain => self.current_page,
            VirtualPlaybackSurfacePageMode::Pinned { page } => page,
        }
    }
}

fn zone_numbers(zone: &VirtualPlaybackExclusionZone) -> Vec<u16> {
    zone.slots
        .iter()
        .map(|slot| 1_000 + slot)
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
