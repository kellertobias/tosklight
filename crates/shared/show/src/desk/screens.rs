use super::DeskStore;
use crate::{
    FixedScreenPane, ProgrammerControlSurfaceConfiguration, ScreenConfiguration, ScreenContent,
    StoreError,
};
use rusqlite::params;
use std::collections::HashSet;
use uuid::Uuid;

impl DeskStore {
    pub fn screens(&self) -> Result<Vec<ScreenConfiguration>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id,name,layout_json,show_dock,show_playbacks,surface_id,show_page_controls,show_programmer,desired_open,display_id,bounds_json,fullscreen,content_json,not_editable,show_compact_encoders FROM screens ORDER BY name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, bool>(6)?,
                row.get::<_, bool>(7)?,
                row.get::<_, bool>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, bool>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, bool>(13)?,
                row.get::<_, bool>(14)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                name,
                layout,
                show_dock,
                show_playbacks,
                surface_id,
                show_page_controls,
                show_programmer,
                desired_open,
                display_id,
                bounds,
                fullscreen,
                content,
                not_editable,
                show_compact_encoders,
            ) = row?;
            let mut screen = ScreenConfiguration {
                id: Uuid::parse_str(&id)?,
                name,
                layout: serde_json::from_str(&layout)?,
                show_dock,
                show_playbacks,
                surface_id,
                show_page_controls,
                show_programmer,
                desired_open,
                display_id,
                bounds: bounds
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                fullscreen,
                content: serde_json::from_str(&content)?,
                not_editable,
                show_compact_encoders,
            };
            normalize_screen(&mut screen)?;
            Ok(screen)
        })
        .collect()
    }

    pub fn screen(&self, id: Uuid) -> Result<Option<ScreenConfiguration>, StoreError> {
        Ok(self.screens()?.into_iter().find(|screen| screen.id == id))
    }

    pub fn put_screen(
        &self,
        desk_id: Uuid,
        mut screen: ScreenConfiguration,
    ) -> Result<ScreenConfiguration, StoreError> {
        screen.name = screen.name.trim().to_owned();
        if screen.name.is_empty() || screen.name.len() > 80 || screen.surface_id.trim().is_empty() {
            return Err(StoreError::Invalid(
                "Screen name and Playback surface identity are required".into(),
            ));
        }
        if !self
            .playback_surfaces(desk_id)?
            .contains_key(&screen.surface_id)
        {
            return Err(StoreError::Invalid(format!(
                "Unknown Playback surface {}",
                screen.surface_id
            )));
        }
        normalize_screen(&mut screen)?;
        self.conn.execute("INSERT INTO screens(id,name,layout_json,show_dock,show_playbacks,surface_id,show_page_controls,show_programmer,desired_open,display_id,bounds_json,fullscreen,content_json,not_editable,show_compact_encoders) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(id) DO UPDATE SET name=excluded.name,layout_json=excluded.layout_json,show_dock=excluded.show_dock,show_playbacks=excluded.show_playbacks,surface_id=excluded.surface_id,show_page_controls=excluded.show_page_controls,show_programmer=excluded.show_programmer,desired_open=excluded.desired_open,display_id=excluded.display_id,bounds_json=excluded.bounds_json,fullscreen=excluded.fullscreen,content_json=excluded.content_json,not_editable=excluded.not_editable,show_compact_encoders=excluded.show_compact_encoders", params![screen.id.to_string(),screen.name,serde_json::to_string(&screen.layout)?,screen.show_dock,screen.show_playbacks,screen.surface_id,screen.show_page_controls,screen.show_programmer,screen.desired_open,screen.display_id,screen.bounds.as_ref().map(serde_json::to_string).transpose()?,screen.fullscreen,serde_json::to_string(&screen.content)?,screen.not_editable,screen.show_compact_encoders])?;
        self.screen(screen.id)?
            .ok_or_else(|| StoreError::Invalid("screen disappeared after update".into()))
    }

    pub fn delete_screen(&self, id: Uuid) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM screens WHERE id=?1", [id.to_string()])?;
        Ok(())
    }

    pub fn programmer_control_surface(
        &self,
    ) -> Result<ProgrammerControlSurfaceConfiguration, StoreError> {
        let (owner_screen_id, visible_encoders) = self.conn.query_row(
            "SELECT owner_screen_id,visible_encoders FROM programmer_control_surface WHERE singleton=1",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, u8>(1)?)),
        )?;
        let owner_screen_id = owner_screen_id
            .map(|value| Uuid::parse_str(&value))
            .transpose()?;
        Ok(ProgrammerControlSurfaceConfiguration {
            owner_screen_id,
            visible_encoders,
        })
    }

    pub fn put_programmer_control_surface(
        &self,
        configuration: ProgrammerControlSurfaceConfiguration,
    ) -> Result<ProgrammerControlSurfaceConfiguration, StoreError> {
        if !matches!(configuration.visible_encoders, 4 | 5 | 6) {
            return Err(StoreError::Invalid(
                "visible encoder count must be 4, 5 or 6".into(),
            ));
        }
        if let Some(owner_screen_id) = configuration.owner_screen_id
            && self.screen(owner_screen_id)?.is_none()
        {
            return Err(StoreError::Invalid(
                "programmer control owner screen does not exist".into(),
            ));
        }
        self.conn.execute(
            "UPDATE programmer_control_surface SET owner_screen_id=?1,visible_encoders=?2 WHERE singleton=1",
            params![
                configuration.owner_screen_id.map(|id| id.to_string()),
                configuration.visible_encoders
            ],
        )?;
        self.programmer_control_surface()
    }
}

fn normalize_screen(screen: &mut ScreenConfiguration) -> Result<(), StoreError> {
    let pane = match &screen.content {
        ScreenContent::Desktop => return Ok(()),
        ScreenContent::ControlSurface => {
            screen.show_dock = false;
            return Ok(());
        }
        ScreenContent::FixedPane { pane } => {
            screen.show_dock = false;
            pane
        }
        ScreenContent::FixedSidePane {
            pane,
            width_percent,
            ..
        } => {
            if !(10..=80).contains(width_percent) {
                return Err(StoreError::Invalid(
                    "fixed side pane width must be within 10-80 percent".into(),
                ));
            }
            screen.show_dock = false;
            pane
        }
    };
    match pane {
        FixedScreenPane::FixtureSheet { columns, .. } => {
            if columns.is_empty() || columns.len() > 12 {
                return Err(StoreError::Invalid(
                    "fixed Fixture Sheet requires at least one valid column".into(),
                ));
            }
            let unique = columns.iter().copied().collect::<HashSet<_>>();
            if unique.len() != columns.len() {
                return Err(StoreError::Invalid(
                    "fixed Fixture Sheet columns must be unique".into(),
                ));
            }
        }
        FixedScreenPane::Stage3d {
            environment_brightness,
            ..
        } if !environment_brightness.is_finite()
            || !(0.0..=1.0).contains(environment_brightness) =>
        {
            return Err(StoreError::Invalid(
                "fixed Stage 3D environment brightness must be within 0-1".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}
