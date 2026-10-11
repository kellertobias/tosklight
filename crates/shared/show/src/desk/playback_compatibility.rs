use crate::StoreError;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeskPlaybackIncompatibility {
    pub message: String,
}

/// Call before writable opening. Recovery must preserve the original database and sidecars,
/// then initialize a separate desk; this audit never upgrades or translates old surface data.
pub fn inspect_desk_playback_contract(
    path: &Path,
) -> Result<Option<DeskPlaybackIncompatibility>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has_schema = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_info'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    let version = if has_schema {
        connection
            .query_row("SELECT version FROM schema_info", [], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?
    } else {
        None
    };
    if version == Some(super::DESK_SCHEMA_VERSION) {
        return Ok(None);
    }
    if version.is_some_and(|value| value > super::DESK_SCHEMA_VERSION) {
        return Ok(Some(DeskPlaybackIncompatibility {
            message: format!(
                "This desk configuration requires schema {} but this build supports {}. The original must remain unchanged; continue in a separate fresh desk configuration.",
                version.unwrap(),
                super::DESK_SCHEMA_VERSION
            ),
        }));
    }
    let has_data = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(has_data.then(|| DeskPlaybackIncompatibility {
        message: "This desk configuration predates unified Playback surfaces (TL-714) and cannot be loaded. The original desk database must be preserved unchanged. Continue in a separate fresh desk configuration; open a compatible show or create a separate empty show.".into(),
    }))
}
