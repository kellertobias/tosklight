use super::{DeskStore, validate_playback_surface};
use crate::{ScreenConfiguration, StoreError};
use light_core::ShowId;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl DeskStore {
    pub fn screens(&self) -> Result<Vec<ScreenConfiguration>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,layout_json,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds_json,fullscreen,playback_layout_json FROM screens ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, u8>(5)?,
                row.get::<_, u8>(6)?,
                row.get::<_, u8>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, bool>(9)?,
                row.get::<_, bool>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, bool>(13)?,
                row.get::<_, Option<String>>(14)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                name,
                layout,
                show_dock,
                show_playbacks,
                playback_count,
                playback_rows,
                first_playback_slot,
                page_mode,
                show_page_controls,
                desired_open,
                display_id,
                bounds,
                fullscreen,
                playback_layout,
            ) = row?;
            Ok(ScreenConfiguration {
                id: Uuid::parse_str(&id)?,
                name,
                layout: serde_json::from_str(&layout)?,
                show_dock,
                show_playbacks,
                playback_count,
                playback_rows,
                first_playback_slot,
                page_mode,
                show_page_controls,
                desired_open,
                display_id,
                bounds: bounds
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                fullscreen,
                playback_layout: playback_layout
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
            })
        })
        .collect()
    }

    pub fn screen(&self, id: Uuid) -> Result<Option<ScreenConfiguration>, StoreError> {
        Ok(self.screens()?.into_iter().find(|screen| screen.id == id))
    }

    pub fn put_screen(
        &self,
        mut screen: ScreenConfiguration,
    ) -> Result<ScreenConfiguration, StoreError> {
        if screen.playback_layout.is_none() {
            screen.playback_layout = self
                .screen(screen.id)?
                .and_then(|existing| existing.playback_layout);
        }
        screen.name = screen.name.trim().to_owned();
        if screen.name.is_empty()
            || screen.name.len() > 80
            || screen.page_mode != "follow_main" && screen.page_mode != "independent"
            || screen.playback_count == 0
            || screen.playback_rows == 0
            || screen.playback_rows > screen.playback_count
            || screen.first_playback_slot == 0
            || u16::from(screen.first_playback_slot) + u16::from(screen.playback_count) - 1 > 127
        {
            return Err(StoreError::Invalid("invalid screen configuration".into()));
        }
        if let Some(layout) = &screen.playback_layout {
            validate_playback_surface(layout)?;
        }
        self.conn.execute("INSERT INTO screens(id,name,layout_json,show_dock,show_playbacks,playback_count,playback_rows,first_playback_slot,page_mode,show_page_controls,desired_open,display_id,bounds_json,fullscreen,playback_layout_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(id) DO UPDATE SET name=excluded.name,layout_json=excluded.layout_json,show_dock=excluded.show_dock,show_playbacks=excluded.show_playbacks,playback_count=excluded.playback_count,playback_rows=excluded.playback_rows,first_playback_slot=excluded.first_playback_slot,page_mode=excluded.page_mode,show_page_controls=excluded.show_page_controls,desired_open=excluded.desired_open,display_id=excluded.display_id,bounds_json=excluded.bounds_json,fullscreen=excluded.fullscreen,playback_layout_json=excluded.playback_layout_json",params![screen.id.to_string(),screen.name,serde_json::to_string(&screen.layout)?,screen.show_dock,screen.show_playbacks,screen.playback_count,screen.playback_rows,screen.first_playback_slot,screen.page_mode,screen.show_page_controls,screen.desired_open,screen.display_id,screen.bounds.as_ref().map(serde_json::to_string).transpose()?,screen.fullscreen,screen.playback_layout.as_ref().map(serde_json::to_string).transpose()?])?;
        self.screen(screen.id)?
            .ok_or_else(|| StoreError::Invalid("screen update failed".into()))
    }

    pub fn delete_screen(&self, id: Uuid) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM screens WHERE id=?1", [id.to_string()])?;
        Ok(())
    }

    pub fn screen_page(&self, screen: Uuid, show: ShowId) -> Result<u8, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT page FROM screen_pages WHERE screen_id=?1 AND show_id=?2",
                params![screen.to_string(), show.0.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(1))
    }

    pub fn set_screen_page(&self, screen: Uuid, show: ShowId, page: u8) -> Result<(), StoreError> {
        if !(1..=127).contains(&page) {
            return Err(StoreError::Invalid("page must be within 1-127".into()));
        }
        self.conn.execute("INSERT INTO screen_pages(screen_id,show_id,page) VALUES(?1,?2,?3) ON CONFLICT(screen_id,show_id) DO UPDATE SET page=excluded.page",params![screen.to_string(),show.0.to_string(),page])?;
        Ok(())
    }
}
