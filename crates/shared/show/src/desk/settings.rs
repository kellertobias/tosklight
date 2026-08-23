use super::DeskStore;
use crate::{ShowEntry, StoreError};
use light_core::ShowId;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl DeskStore {
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(Into::into)
    }

    /// Every setting whose key starts with `prefix`, in key order.
    ///
    /// Used to fold settings that were once stored per desk into one installation-wide value.
    pub fn settings_with_prefix(&self, prefix: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT key,value FROM settings WHERE key LIKE ?1 ORDER BY key")?;
        let rows = statement
            .query_map([format!("{prefix}%")], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Remove a setting outright, as retiring a per-desk key does once it has been folded in.
    pub fn remove_setting(&self, key: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM settings WHERE key=?1", [key])?;
        Ok(())
    }

    pub fn set_active_show(&self, id: Option<ShowId>) -> Result<(), StoreError> {
        match id {
            Some(id) => self.set_setting("active_show_id", &id.0.to_string()),
            None => {
                self.conn
                    .execute("DELETE FROM settings WHERE key='active_show_id'", [])?;
                Ok(())
            }
        }
    }

    pub fn active_show(&self) -> Result<Option<ShowEntry>, StoreError> {
        let Some(id) = self.setting("active_show_id")? else {
            return Ok(None);
        };
        self.show(ShowId(Uuid::parse_str(&id)?))
    }
}
