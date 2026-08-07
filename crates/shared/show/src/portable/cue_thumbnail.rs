//! Persisted cue preview pictures.
//!
//! A cue preview is a picture of the look the cue leaves on stage, drawn once when the operator
//! records or edits the cue rather than every time a desk opens the cue list. It lives inside the
//! portable show file so a show carried to another desk arrives with its previews already drawn.
//!
//! The pictures are derived data. Losing them costs a redraw, never operator work, so every read
//! path here treats an absent row as "not drawn yet" instead of an error.

use crate::{ShowStore, StoreError};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{OptionalExtension, params};

use super::repository::immediate_transaction;

/// Largest accepted picture. A 240x135 WebP preview is a few kilobytes; this bound only exists so
/// a malformed or hostile upload cannot grow the show file without limit.
const MAX_IMAGE_BYTES: usize = 512 * 1024;
const MAX_DIMENSION: u32 = 1_024;
const MAX_HASH_BYTES: usize = 128;

/// One stored preview picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueThumbnail {
    pub cue_id: String,
    pub image: Vec<u8>,
    /// Opaque tag describing the desk state the picture was drawn from. The desk compares it
    /// against the state it currently holds to decide whether the picture still tells the truth.
    pub state_hash: String,
    pub width: u32,
    pub height: u32,
    pub updated_at: DateTime<Utc>,
}

/// What a desk needs to decide whether a stored picture is still current, without moving pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CueThumbnailEntry {
    pub cue_id: String,
    pub state_hash: String,
    pub updated_at: DateTime<Utc>,
}

impl ShowStore {
    /// Lists what is stored, so a desk can fetch only the pictures it is missing or that went
    /// stale.
    pub fn cue_thumbnail_index(&self) -> Result<Vec<CueThumbnailEntry>, StoreError> {
        let mut statement = self
            .conn
            .prepare("SELECT cue_id,state_hash,updated_at FROM cue_thumbnails ORDER BY cue_id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (cue_id, state_hash, updated_at) = row?;
            entries.push(CueThumbnailEntry {
                cue_id,
                state_hash,
                updated_at: decode_timestamp(&updated_at)?,
            });
        }
        Ok(entries)
    }

    pub fn cue_thumbnail(&self, cue_id: &str) -> Result<Option<CueThumbnail>, StoreError> {
        self.conn
            .query_row(
                "SELECT cue_id,image,state_hash,width,height,updated_at FROM cue_thumbnails WHERE cue_id=?1",
                params![cue_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?
            .map(decode)
            .transpose()
    }

    /// Stores a batch of pictures in one transaction.
    ///
    /// Editing an early cue redraws every cue that tracks from it, so the desk arrives with the
    /// whole run at once rather than one request per cue.
    pub fn put_cue_thumbnails(&self, thumbnails: &[CueThumbnail]) -> Result<usize, StoreError> {
        for thumbnail in thumbnails {
            validate(thumbnail)?;
        }
        let tx = immediate_transaction(&self.conn)?;
        {
            let mut statement = tx.prepare(
                "INSERT INTO cue_thumbnails(cue_id,image,state_hash,width,height,updated_at) VALUES(?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(cue_id) DO UPDATE SET image=excluded.image,state_hash=excluded.state_hash,width=excluded.width,height=excluded.height,updated_at=excluded.updated_at",
            )?;
            for thumbnail in thumbnails {
                statement.execute(params![
                    thumbnail.cue_id,
                    thumbnail.image,
                    thumbnail.state_hash,
                    i64::from(thumbnail.width),
                    i64::from(thumbnail.height),
                    thumbnail
                        .updated_at
                        .to_rfc3339_opts(SecondsFormat::Millis, true),
                ])?;
            }
        }
        tx.commit()?;
        Ok(thumbnails.len())
    }

    /// Drops pictures for cues that no longer exist.
    ///
    /// `live_cue_ids` is every cue id still present anywhere in the show. A cue deleted from one
    /// list and recorded into another keeps its picture because its id survives.
    pub fn prune_cue_thumbnails(&self, live_cue_ids: &[String]) -> Result<usize, StoreError> {
        let tx = immediate_transaction(&self.conn)?;
        let removed = {
            let stored: Vec<String> = {
                let mut statement = tx.prepare("SELECT cue_id FROM cue_thumbnails")?;
                let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let mut removed = 0;
            let mut statement = tx.prepare("DELETE FROM cue_thumbnails WHERE cue_id=?1")?;
            for cue_id in stored {
                if !live_cue_ids.iter().any(|live| live == &cue_id) {
                    removed += statement.execute(params![cue_id])?;
                }
            }
            removed
        };
        tx.commit()?;
        Ok(removed)
    }
}

fn validate(thumbnail: &CueThumbnail) -> Result<(), StoreError> {
    if thumbnail.cue_id.trim().is_empty() {
        return Err(StoreError::Invalid("cue_id must not be empty".into()));
    }
    if thumbnail.image.is_empty() || thumbnail.image.len() > MAX_IMAGE_BYTES {
        return Err(StoreError::Invalid(format!(
            "cue preview image must contain 1-{MAX_IMAGE_BYTES} bytes"
        )));
    }
    if thumbnail.state_hash.trim().is_empty() || thumbnail.state_hash.len() > MAX_HASH_BYTES {
        return Err(StoreError::Invalid(format!(
            "cue preview state_hash must contain 1-{MAX_HASH_BYTES} bytes"
        )));
    }
    if thumbnail.width == 0
        || thumbnail.height == 0
        || thumbnail.width > MAX_DIMENSION
        || thumbnail.height > MAX_DIMENSION
    {
        return Err(StoreError::Invalid(format!(
            "cue preview dimensions must be 1-{MAX_DIMENSION} pixels"
        )));
    }
    Ok(())
}

fn decode(
    (cue_id, image, state_hash, width, height, updated_at): (
        String,
        Vec<u8>,
        String,
        i64,
        i64,
        String,
    ),
) -> Result<CueThumbnail, StoreError> {
    Ok(CueThumbnail {
        cue_id,
        image,
        state_hash,
        width: u32::try_from(width)
            .map_err(|_| StoreError::Invalid("cue preview width is out of range".into()))?,
        height: u32::try_from(height)
            .map_err(|_| StoreError::Invalid("cue preview height is out of range".into()))?,
        updated_at: decode_timestamp(&updated_at)?,
    })
}

fn decode_timestamp(value: &str) -> Result<DateTime<Utc>, StoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|_| {
            StoreError::Invalid(format!("cue preview updated_at is not RFC 3339: {value}"))
        })
}
