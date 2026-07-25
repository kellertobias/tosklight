use super::{DeskStore, validate_playback_surface};
use crate::{ClientDesk, ControlDesk, PlaybackSurfaceLayout, StoreError};
use chrono::Utc;
use light_core::ShowId;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl DeskStore {
    pub fn desks(&self) -> Result<Vec<ControlDesk>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,osc_alias,columns_count,rows_count,buttons_count,playback_layout_json FROM control_desks ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, u8>(3)?,
                row.get::<_, u8>(4)?,
                row.get::<_, u8>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, osc_alias, columns, rows, buttons, playback_layout) = row?;
            Ok(ControlDesk {
                id: Uuid::parse_str(&id)?,
                name,
                osc_alias,
                columns,
                rows,
                buttons,
                playback_layout: playback_layout
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
            })
        })
        .collect()
    }

    pub fn control_desk(&self, id: Uuid) -> Result<Option<ControlDesk>, StoreError> {
        Ok(self.desks()?.into_iter().find(|desk| desk.id == id))
    }

    pub fn control_desk_by_alias(&self, alias: &str) -> Result<Option<ControlDesk>, StoreError> {
        Ok(self
            .desks()?
            .into_iter()
            .find(|desk| desk.osc_alias.eq_ignore_ascii_case(alias)))
    }

    pub fn client_desks(&self) -> Result<Vec<ClientDesk>, StoreError> {
        let desks = self.desks()?;
        desks
            .into_iter()
            .map(|desk| {
                let (client_id, last_connected_at) = self.conn.query_row(
                    "SELECT client_id,last_connected_at FROM control_desks WHERE id=?1",
                    [desk.id.to_string()],
                    |row| Ok((row.get::<_, Option<String>>(0)?, row.get(1)?)),
                )?;
                Ok(ClientDesk {
                    client_id: client_id.map(|value| Uuid::parse_str(&value)).transpose()?,
                    last_connected_at,
                    desk,
                })
            })
            .collect()
    }

    pub fn resolve_client_desk(
        &self,
        client_id: Uuid,
        remembered_desk_id: Option<Uuid>,
    ) -> Result<ControlDesk, StoreError> {
        let now = Utc::now().to_rfc3339();
        if let Some(home) = self
            .client_desks()?
            .into_iter()
            .find(|entry| entry.client_id == Some(client_id))
        {
            self.conn.execute(
                "UPDATE control_desks SET last_connected_at=?1 WHERE client_id=?2",
                params![now, client_id.to_string()],
            )?;
            return Ok(remembered_desk_id
                .and_then(|id| self.control_desk(id).ok().flatten())
                .unwrap_or(home.desk));
        }
        if let Some(remembered) = remembered_desk_id
            && let Some(candidate) = self
                .client_desks()?
                .into_iter()
                .find(|entry| entry.desk.id == remembered && entry.client_id.is_none())
        {
            self.conn.execute(
                "UPDATE control_desks SET client_id=?1,last_connected_at=?2 WHERE id=?3 AND client_id IS NULL",
                params![client_id.to_string(), now, candidate.desk.id.to_string()],
            )?;
            return Ok(candidate.desk);
        }
        let remembered_default = remembered_desk_id
            .map(|id| self.control_desk(id))
            .transpose()?
            .flatten();
        let suffix = client_id.simple().to_string();
        let desk = self.add_desk(
            &format!("Client {}", &suffix[..6]),
            &format!("desk-{}", &suffix[..8]),
        )?;
        self.conn.execute(
            "UPDATE control_desks SET client_id=?1,last_connected_at=?2 WHERE id=?3",
            params![client_id.to_string(), now, desk.id.to_string()],
        )?;
        Ok(remembered_default.unwrap_or(desk))
    }

    pub fn touch_client(&self, client_id: Uuid) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE control_desks SET last_connected_at=?1 WHERE client_id=?2",
            params![Utc::now().to_rfc3339(), client_id.to_string()],
        )?;
        Ok(())
    }

    pub fn remove_client_desk(&mut self, desk_id: Uuid) -> Result<bool, StoreError> {
        let transaction = self.conn.transaction()?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM control_desks WHERE id=?1",
                [desk_id.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Ok(false);
        }
        let desk_key = desk_id.to_string();
        transaction.execute(
            "DELETE FROM settings WHERE key=?1",
            [format!("desk_lock:{desk_key}")],
        )?;
        let settings = {
            let mut statement = transaction.prepare(
                "SELECT key,value FROM settings WHERE key='server_configuration' OR key LIKE 'virtual_playback_exclusion_zones:%'",
            )?;
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        for (key, encoded) in settings {
            let mut value: serde_json::Value = serde_json::from_str(&encoded)?;
            let changed = if key == "server_configuration" {
                value
                    .get_mut("update_settings_by_desk")
                    .and_then(serde_json::Value::as_object_mut)
                    .is_some_and(|entries| entries.remove(&desk_key).is_some())
            } else {
                value
                    .as_object_mut()
                    .is_some_and(|entries| entries.remove(&desk_key).is_some())
            };
            if changed {
                transaction.execute(
                    "UPDATE settings SET value=?1 WHERE key=?2",
                    params![serde_json::to_string(&value)?, key],
                )?;
            }
        }
        transaction.execute("DELETE FROM control_desks WHERE id=?1", [desk_key])?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn add_desk(&self, name: &str, alias: &str) -> Result<ControlDesk, StoreError> {
        let alias = alias.trim().to_ascii_lowercase();
        if alias.is_empty()
            || alias.len() > 40
            || !alias
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(StoreError::Invalid(
                "OSC alias must contain only letters, numbers, dash, or underscore".into(),
            ));
        }
        let desk = ControlDesk {
            id: Uuid::new_v4(),
            name: name.trim().to_owned(),
            osc_alias: alias,
            columns: 8,
            rows: 1,
            buttons: 3,
            playback_layout: None,
        };
        self.conn.execute("INSERT INTO control_desks(id,name,osc_alias,columns_count,rows_count,buttons_count) VALUES (?1,?2,?3,?4,?5,?6)",params![desk.id.to_string(),desk.name,desk.osc_alias,desk.columns,desk.rows,desk.buttons])?;
        Ok(desk)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_desk(
        &self,
        id: Uuid,
        name: &str,
        alias: &str,
        columns: u8,
        rows: u8,
        buttons: u8,
        playback_layout: Option<PlaybackSurfaceLayout>,
    ) -> Result<ControlDesk, StoreError> {
        let alias = alias.trim().to_ascii_lowercase();
        if name.trim().is_empty()
            || alias.is_empty()
            || !alias
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            || !(1..=32).contains(&columns)
            || !(1..=127).contains(&rows)
            || buttons > 3
        {
            return Err(StoreError::Invalid(
                "invalid control desk configuration".into(),
            ));
        }
        let playback_layout = playback_layout.or_else(|| {
            self.control_desk(id)
                .ok()
                .flatten()
                .and_then(|desk| desk.playback_layout)
        });
        if let Some(layout) = &playback_layout {
            validate_playback_surface(layout)?;
        }
        if self.conn.execute("UPDATE control_desks SET name=?1,osc_alias=?2,columns_count=?3,rows_count=?4,buttons_count=?5,playback_layout_json=?6 WHERE id=?7",params![name.trim(),alias,columns,rows,buttons,playback_layout.as_ref().map(serde_json::to_string).transpose()?,id.to_string()])?!=1{return Err(StoreError::Invalid("control desk does not exist".into()));}
        self.control_desk(id)?
            .ok_or_else(|| StoreError::Invalid("control desk update failed".into()))
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
