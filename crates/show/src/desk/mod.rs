mod control_desks;
mod migration;
mod screens;
mod settings;
mod show_library;
mod users;

use crate::{PlaybackSurfaceLayout, StoreError, connection::configure};
use rusqlite::Connection;
use std::path::Path;

pub(crate) const DESK_SCHEMA_VERSION: i64 = 9;

pub struct DeskStore {
    pub(crate) conn: Connection,
}

impl DeskStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        migration::migrate_desk(&mut conn)?;
        let store = Self { conn };
        if store.users()?.is_empty() {
            store.add_user("Operator")?;
        }
        Ok(store)
    }
}

fn validate_playback_surface(layout: &PlaybackSurfaceLayout) -> Result<(), StoreError> {
    if !(1..=32).contains(&layout.playbacks_per_row)
        || layout.rows.is_empty()
        || layout.rows.len() > 127
        || usize::from(layout.playbacks_per_row) * layout.rows.len() > 127
    {
        return Err(StoreError::Invalid(
            "invalid playback surface layout".into(),
        ));
    }
    for row in &layout.rows {
        if row.first_playback_slot == 0
            || row.button_count > 3
            || u16::from(row.first_playback_slot) + u16::from(layout.playbacks_per_row) - 1 > 127
        {
            return Err(StoreError::Invalid("invalid playback surface row".into()));
        }
    }
    Ok(())
}
