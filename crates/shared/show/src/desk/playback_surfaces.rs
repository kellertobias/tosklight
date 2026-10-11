use super::{DeskStore, validate_playback_surface};
use crate::{
    PlaybackControlPresentation, PlaybackPageSource, PlaybackSurfaceConfiguration,
    PlaybackSurfaceLayout, PlaybackSurfaceRow, StoreError,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const SURFACES: &str = "playback_surfaces";
const SELECTED: &str = "selected_playback_surface";

pub type PlaybackSurfaceRegistry = BTreeMap<String, PlaybackSurfaceConfiguration>;

fn default_surfaces() -> PlaybackSurfaceRegistry {
    BTreeMap::from([(
        "main".into(),
        PlaybackSurfaceConfiguration {
            name: "Main".into(),
            page_source: PlaybackPageSource::Own { page: 1 },
            layout: PlaybackSurfaceLayout {
                rows: vec![
                    PlaybackSurfaceRow {
                        first_position: 1,
                        count: 15,
                        presentation: PlaybackControlPresentation::Fader,
                        button_count: 3,
                    },
                    PlaybackSurfaceRow {
                        first_position: 101,
                        count: 15,
                        presentation: PlaybackControlPresentation::Buttons,
                        button_count: 1,
                    },
                ],
            },
        },
    )])
}

pub fn resolve_playback_surface_pages(
    surfaces: &PlaybackSurfaceRegistry,
) -> Result<BTreeMap<String, u8>, StoreError> {
    surfaces
        .keys()
        .map(|id| resolve_page(surfaces, id, &mut BTreeSet::new()).map(|page| (id.clone(), page)))
        .collect()
}

fn resolve_page(
    surfaces: &PlaybackSurfaceRegistry,
    id: &str,
    visited: &mut BTreeSet<String>,
) -> Result<u8, StoreError> {
    if !visited.insert(id.to_owned()) {
        return Err(StoreError::Invalid(format!(
            "Playback page following has a cycle at {id}"
        )));
    }
    let surface = surfaces
        .get(id)
        .ok_or_else(|| StoreError::Invalid(format!("Unknown Playback surface {id}")))?;
    match &surface.page_source {
        PlaybackPageSource::Own { page } | PlaybackPageSource::Pinned { page } => {
            if (1..=99).contains(page) {
                Ok(*page)
            } else {
                Err(StoreError::Invalid(
                    "Playback page must be within 1-99".into(),
                ))
            }
        }
        PlaybackPageSource::Follow { surface_id } => resolve_page(surfaces, surface_id, visited),
    }
}

impl DeskStore {
    pub fn playback_surfaces(&self, desk_id: Uuid) -> Result<PlaybackSurfaceRegistry, StoreError> {
        let surfaces = match self.setting(&format!("{SURFACES}:{desk_id}"))? {
            Some(serialized) => serde_json::from_str(&serialized)?,
            None => default_surfaces(),
        };
        validate_surfaces(&surfaces)?;
        Ok(surfaces)
    }

    pub fn put_playback_surfaces(
        &self,
        desk_id: Uuid,
        surfaces: &PlaybackSurfaceRegistry,
    ) -> Result<(), StoreError> {
        validate_surfaces(surfaces)?;
        let selected = self.selected_playback_surface(desk_id)?;
        if !surfaces.contains_key(&selected) {
            return Err(StoreError::Invalid(
                "The selected Playback surface cannot be removed before selecting another surface"
                    .into(),
            ));
        }
        self.set_setting(
            &format!("{SURFACES}:{desk_id}"),
            &serde_json::to_string(surfaces)?,
        )
    }

    pub fn selected_playback_surface(&self, desk_id: Uuid) -> Result<String, StoreError> {
        Ok(self
            .setting(&format!("{SELECTED}:{desk_id}"))?
            .unwrap_or_else(|| "main".into()))
    }

    pub fn select_playback_surface(
        &self,
        desk_id: Uuid,
        surface_id: &str,
    ) -> Result<(), StoreError> {
        if !self.playback_surfaces(desk_id)?.contains_key(surface_id) {
            return Err(StoreError::Invalid(format!(
                "Unknown Playback surface {surface_id}"
            )));
        }
        self.set_setting(&format!("{SELECTED}:{desk_id}"), surface_id)
    }
}

fn validate_surfaces(surfaces: &PlaybackSurfaceRegistry) -> Result<(), StoreError> {
    if !surfaces.contains_key("main") {
        return Err(StoreError::Invalid(
            "The Main Playback surface is required".into(),
        ));
    }
    for (id, surface) in surfaces {
        if id.is_empty() || surface.name.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Playback surface identity and name are required".into(),
            ));
        }
        validate_playback_surface(&surface.layout)?;
    }
    resolve_playback_surface_pages(surfaces).map(|_| ())
}
