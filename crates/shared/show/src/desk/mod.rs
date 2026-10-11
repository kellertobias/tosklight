pub(super) mod control_desks;
mod migration;
mod playback_compatibility;
mod playback_surfaces;
mod screens;
mod sessions;
mod settings;
pub use playback_compatibility::{DeskPlaybackIncompatibility, inspect_desk_playback_contract};
pub use playback_surfaces::{PlaybackSurfaceRegistry, resolve_playback_surface_pages};
mod show_library;

use crate::{PlaybackSurfaceLayout, StoreError, connection::configure};
use rusqlite::Connection;
use std::path::Path;

pub(crate) const DESK_SCHEMA_VERSION: i64 = 17;

pub struct DeskStore {
    pub(crate) conn: Connection,
}

impl DeskStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        if let Some(rejection) = inspect_desk_playback_contract(path.as_ref())? {
            return Err(StoreError::Invalid(rejection.message));
        }
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migration::migrate_desk(&mut conn)?;
        Ok(Self { conn })
    }
}

fn validate_playback_surface(layout: &PlaybackSurfaceLayout) -> Result<(), StoreError> {
    if layout.rows.is_empty() || layout.rows.len() > 1000 {
        return Err(StoreError::Invalid(
            "Playback surface requires 1-1000 rows".into(),
        ));
    }
    for row in &layout.rows {
        if row.first_position == 0
            || row.count == 0
            || row.button_count > 3
            || u32::from(row.first_position) + u32::from(row.count) - 1 > 1000
        {
            return Err(StoreError::Invalid(
                "Playback row must address positions 1-1000 with 0-3 buttons".into(),
            ));
        }
    }
    Ok(())
}
