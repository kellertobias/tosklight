use super::{DeskStore, validate_playback_surface};
use crate::{ClientDesk, ControlDesk, PlaybackSurfaceLayout, StoreError};
use chrono::Utc;
use light_core::ShowId;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

/// The name a desk is created under when an installation has none.
///
/// Deliberately fixed. A desk used to be named after the client that first connected, which is how
/// an installation ended up with a row per window.
const DESK_NAME: &str = "Desk";

impl DeskStore {
    /// The desk. There is one; it is created on first read if the installation has none.
    pub fn desk(&self) -> Result<ControlDesk, StoreError> {
        if let Some(desk) = self.stored_desk()? {
            return Ok(desk);
        }
        self.create_desk()
    }

    /// The desk, if this installation has stored one yet.
    ///
    /// Separate from [`Self::desk`] so a read can stay a read: answering what an installation holds
    /// must not write a row into it.
    fn stored_desk(&self) -> Result<Option<ControlDesk>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,columns_count,rows_count,buttons_count,playback_layout_json FROM control_desks ORDER BY name COLLATE NOCASE")?;
        let mut rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get(1)?,
                row.get::<_, u8>(2)?,
                row.get::<_, u8>(3)?,
                row.get::<_, u8>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?;
        let Some(row) = rows.next() else {
            return Ok(None);
        };
        let (id, name, columns, rows, buttons, playback_layout) = row?;
        Ok(Some(ControlDesk {
            id: Uuid::parse_str(&id)?,
            name,
            columns,
            rows,
            buttons,
            playback_layout: playback_layout
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
        }))
    }

    /// Every window that has connected, each shown against the desk it operates.
    ///
    /// A client is a window, not a desk. Both used to live on the same row, which is why opening
    /// a second window created a second control desk — and why two browser screens then filtered
    /// each other's events away. They are separate records now: one desk, and however many
    /// clients have connected to it.
    pub fn client_desks(&self) -> Result<Vec<ClientDesk>, StoreError> {
        let Some(desk) = self.stored_desk()? else {
            return Ok(Vec::new());
        };
        let mut statement = self
            .conn
            .prepare("SELECT client_id,last_connected_at FROM desk_clients ORDER BY client_id")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.is_empty() {
            return Ok(vec![ClientDesk {
                client_id: None,
                last_connected_at: None,
                desk,
            }]);
        }
        rows.into_iter()
            .map(|(client_id, last_connected_at)| {
                Ok(ClientDesk {
                    client_id: Some(Uuid::parse_str(&client_id)?),
                    last_connected_at: (!last_connected_at.is_empty()).then_some(last_connected_at),
                    desk: desk.clone(),
                })
            })
            .collect()
    }

    /// Register a connecting window and answer with the desk it operates.
    ///
    /// There is one desk. This used to create a control desk per client, which is why an
    /// installation from before the collapse holds a row per window that ever connected. The
    /// window is remembered in its own record instead, so a window is still known without a desk
    /// appearing behind it, and a desk record from before the collapse is honoured rather than
    /// discarded, so saved screen configuration keeps working.
    pub fn resolve_client_desk(&self, client_id: Uuid) -> Result<ControlDesk, StoreError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO desk_clients(client_id,last_connected_at) VALUES(?1,?2) \
             ON CONFLICT(client_id) DO UPDATE SET last_connected_at=excluded.last_connected_at",
            params![client_id.to_string(), now],
        )?;
        self.desk()
    }

    pub fn touch_client(&self, client_id: Uuid) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE desk_clients SET last_connected_at=?1 WHERE client_id=?2",
            params![Utc::now().to_rfc3339(), client_id.to_string()],
        )?;
        Ok(())
    }

    /// Forget one window that has connected.
    ///
    /// A client used to be a desk, so removing it removed the desk and everything scoped to it.
    /// A client is a window on the one desk now, so forgetting it removes only its registration —
    /// the desk, its lock, its per-show page and playback selection all belong to the desk and
    /// outlive any single window.
    pub fn remove_client(&mut self, client_id: Uuid) -> Result<bool, StoreError> {
        Ok(self.conn.execute(
            "DELETE FROM desk_clients WHERE client_id=?1",
            [client_id.to_string()],
        )? == 1)
    }

    /// Write a second desk row, as an installation from before the collapse holds.
    ///
    /// Test-only on purpose: nothing in the desk may create a second one, and the migration that
    /// collapses them still has to be exercised against a database that has them.
    #[cfg(test)]
    pub(crate) fn insert_legacy_desk(&self, name: &str) -> Result<ControlDesk, StoreError> {
        self.insert_desk(name)
    }

    fn create_desk(&self) -> Result<ControlDesk, StoreError> {
        self.insert_desk(DESK_NAME)
    }

    fn insert_desk(&self, name: &str) -> Result<ControlDesk, StoreError> {
        let desk = ControlDesk {
            id: Uuid::new_v4(),
            name: name.trim().to_owned(),
            columns: 8,
            rows: 1,
            buttons: 3,
            playback_layout: None,
        };
        self.conn.execute("INSERT INTO control_desks(id,name,columns_count,rows_count,buttons_count) VALUES (?1,?2,?3,?4,?5)",params![desk.id.to_string(),desk.name,desk.columns,desk.rows,desk.buttons])?;
        Ok(desk)
    }

    pub fn update_desk(
        &self,
        id: Uuid,
        name: &str,
        columns: u8,
        rows: u8,
        buttons: u8,
        playback_layout: Option<PlaybackSurfaceLayout>,
    ) -> Result<ControlDesk, StoreError> {
        if name.trim().is_empty()
            || !(1..=32).contains(&columns)
            || !(1..=127).contains(&rows)
            || buttons > 3
        {
            return Err(StoreError::Invalid(
                "invalid control desk configuration".into(),
            ));
        }
        let playback_layout =
            playback_layout.or_else(|| self.desk().ok().and_then(|desk| desk.playback_layout));
        if let Some(layout) = &playback_layout {
            validate_playback_surface(layout)?;
        }
        if self.conn.execute("UPDATE control_desks SET name=?1,columns_count=?2,rows_count=?3,buttons_count=?4,playback_layout_json=?5 WHERE id=?6",params![name.trim(),columns,rows,buttons,playback_layout.as_ref().map(serde_json::to_string).transpose()?,id.to_string()])?!=1{return Err(StoreError::Invalid("control desk does not exist".into()));}
        self.desk()
    }

    pub fn desk_page(&self, desk: Uuid, show: ShowId) -> Result<u8, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT page FROM control_desk_pages WHERE desk_id=?1 AND show_id=?2",
                params![desk.to_string(), show.0.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(1))
    }

    pub fn set_desk_page(&self, desk: Uuid, show: ShowId, page: u8) -> Result<(), StoreError> {
        if !(1..=127).contains(&page) {
            return Err(StoreError::Invalid("page must be within 1-127".into()));
        }
        self.conn.execute("INSERT INTO control_desk_pages(desk_id,show_id,page) VALUES (?1,?2,?3) ON CONFLICT(desk_id,show_id) DO UPDATE SET page=excluded.page",params![desk.to_string(),show.0.to_string(),page])?;
        Ok(())
    }

    pub fn selected_playback(&self, desk: Uuid, show: ShowId) -> Result<Option<u16>, StoreError> {
        self.conn
            .query_row(
                "SELECT playback FROM control_desk_selections WHERE desk_id=?1 AND show_id=?2",
                params![desk.to_string(), show.0.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn set_selected_playback(
        &self,
        desk: Uuid,
        show: ShowId,
        playback: Option<u16>,
    ) -> Result<(), StoreError> {
        if playback.is_some_and(|number| !(1..=1_000).contains(&number)) {
            return Err(StoreError::Invalid("playback must be within 1-1000".into()));
        }
        match playback {
            Some(number) => {
                self.conn.execute("INSERT INTO control_desk_selections(desk_id,show_id,playback) VALUES (?1,?2,?3) ON CONFLICT(desk_id,show_id) DO UPDATE SET playback=excluded.playback", params![desk.to_string(),show.0.to_string(),number])?;
            }
            None => {
                self.conn.execute(
                    "DELETE FROM control_desk_selections WHERE desk_id=?1 AND show_id=?2",
                    params![desk.to_string(), show.0.to_string()],
                )?;
            }
        }
        Ok(())
    }
}

/// Delete control desks and the state keyed only by them.
///
/// The desk's own rows cascade or are deleted here; the two settings documents that key entries
/// by desk are rewritten so a removed desk leaves nothing addressing it behind.
pub(super) fn delete_desks(
    transaction: &rusqlite::Transaction<'_>,
    desk_ids: &[String],
) -> Result<(), StoreError> {
    for desk_id in desk_ids {
        transaction.execute("DELETE FROM control_desk_pages WHERE desk_id=?1", [desk_id])?;
        transaction.execute(
            "DELETE FROM control_desk_selections WHERE desk_id=?1",
            [desk_id],
        )?;
        transaction.execute(
            "DELETE FROM settings WHERE key=?1",
            [format!("desk_lock:{desk_id}")],
        )?;
        transaction.execute("DELETE FROM control_desks WHERE id=?1", [desk_id])?;
    }
    let entries = {
        let mut statement = transaction.prepare(
            "SELECT key,value FROM settings WHERE key='server_configuration' OR key LIKE 'virtual_playback_exclusion_zones:%'",
        )?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (key, encoded) in entries {
        let mut value: serde_json::Value = serde_json::from_str(&encoded)?;
        let mut changed = false;
        for desk_id in desk_ids {
            changed |= if key == "server_configuration" {
                value
                    .get_mut("update_settings_by_desk")
                    .and_then(serde_json::Value::as_object_mut)
                    .is_some_and(|entries| entries.remove(desk_id).is_some())
            } else {
                value
                    .as_object_mut()
                    .is_some_and(|entries| entries.remove(desk_id).is_some())
            };
        }
        if changed {
            transaction.execute(
                "UPDATE settings SET value=?1 WHERE key=?2",
                params![serde_json::to_string(&value)?, key],
            )?;
        }
    }
    Ok(())
}
