//! The synchronized part of a show, as plain JSON: what the Architect and Control keep in step.
//!
//! One map holds every synchronized object. Patched fixtures are in it too, keyed by fixture
//! identity under [`PATCHED_FIXTURE_KIND`] and held as the `PatchFixtureInput` document a sync
//! transaction's patch field edits address — the representation the desk compares against — so
//! the stored record's incidental shape (computed heads, legacy embedded definitions) never reads
//! as an edit.

use light_application::show_sync::{
    PATCHED_FIXTURE_KIND, SHOW_SYNC_METADATA_PREFIXES, SHOW_SYNC_OBJECT_KINDS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use viz_document::PlanningDocument;

type Answer<T> = Result<T, String>;

/// One synchronized object's identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ObjectKey {
    pub kind: String,
    pub id: String,
}

impl ObjectKey {
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id: id.into(),
        }
    }

    pub fn is_fixture(&self) -> bool {
        self.kind == PATCHED_FIXTURE_KIND
    }
}

/// The synchronized content of one show at one moment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncState {
    pub objects: BTreeMap<ObjectKey, Value>,
    pub metadata: BTreeMap<String, String>,
}

/// One profile revision a show holds, by identity.
pub type ProfileKey = (uuid::Uuid, u64);

/// A show's synchronized content together with the revisions its store gave each object.
#[derive(Clone, Debug, Default)]
pub struct VersionedState {
    pub state: SyncState,
    pub revisions: BTreeMap<ObjectKey, u64>,
    pub profiles: BTreeSet<ProfileKey>,
    pub show_revision: u64,
}

/// Every object kind kept in step, including patched fixtures.
pub fn synchronized_kinds() -> impl Iterator<Item = &'static str> {
    SHOW_SYNC_OBJECT_KINDS
        .iter()
        .copied()
        .chain(std::iter::once(PATCHED_FIXTURE_KIND))
}

pub fn is_sync_metadata_key(key: &str) -> bool {
    light_application::show_sync::is_sync_metadata_key(key)
}

/// Reads the synchronized content of an open document, decoding only objects whose stored stamp
/// changed since the last read. A gesture is captured by reading the document before and after
/// it, so this must stay cheap on a large rig with large bodies (underlay images, models).
#[derive(Default)]
pub struct DocumentReader {
    cache: BTreeMap<(String, String), Cached>,
}

struct Cached {
    stamp: light_show::ObjectStamp,
    key: ObjectKey,
    body: Value,
}

/// When this many objects of one read are new or changed, reading each kind whole is cheaper
/// than reading them one by one.
const SINGLE_READS: usize = 8;

impl DocumentReader {
    pub fn read(&mut self, document: &PlanningDocument) -> Answer<VersionedState> {
        let mut versioned = VersionedState {
            show_revision: document.portable_revision().map_err(|e| e.to_string())?,
            ..VersionedState::default()
        };
        let kinds: Vec<&str> = synchronized_kinds().collect();
        let stamps = document.object_stamps(&kinds).map_err(|e| e.to_string())?;
        let changed: Vec<&light_show::ObjectStamp> = stamps
            .iter()
            .filter(|stamp| {
                self.cache
                    .get(&(stamp.kind.clone(), stamp.id.clone()))
                    .is_none_or(|cached| &cached.stamp != *stamp)
            })
            .collect();
        let mut fresh = BTreeMap::new();
        if changed.len() > SINGLE_READS {
            let kinds: BTreeSet<&str> = changed.iter().map(|stamp| stamp.kind.as_str()).collect();
            for kind in kinds {
                for object in document.objects(kind).map_err(|e| e.to_string())? {
                    fresh.insert((kind.to_owned(), object.id.clone()), object);
                }
            }
        } else {
            for stamp in &changed {
                if let Some(object) = document
                    .object(&stamp.kind, &stamp.id)
                    .map_err(|e| e.to_string())?
                {
                    fresh.insert((stamp.kind.clone(), stamp.id.clone()), object);
                }
            }
        }
        let mut cache = BTreeMap::new();
        for stamp in stamps {
            let identity = (stamp.kind.clone(), stamp.id.clone());
            let cached = match (self.cache.remove(&identity), fresh.remove(&identity)) {
                (_, Some(object)) => decode(stamp, object)?,
                (Some(cached), None) if cached.stamp == stamp => cached,
                // Written between the stamp read and the body read: the next read sees it.
                _ => continue,
            };
            if cached.key.is_fixture()
                && let Some(profile) = fixture_profile(&cached.body)
            {
                versioned.profiles.insert(profile);
            }
            versioned
                .revisions
                .insert(cached.key.clone(), cached.stamp.revision);
            versioned
                .state
                .objects
                .insert(cached.key.clone(), cached.body.clone());
            cache.insert(identity, cached);
        }
        self.cache = cache;
        versioned.state.metadata = document
            .metadata_with_prefixes(SHOW_SYNC_METADATA_PREFIXES)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|(key, value)| is_sync_metadata_key(key) && !value.is_empty())
            .collect();
        Ok(versioned)
    }
}

fn decode(stamp: light_show::ObjectStamp, object: light_show::VersionedObject) -> Answer<Cached> {
    let (key, body) = if stamp.kind == PATCHED_FIXTURE_KIND {
        fixture_entry(object.body, object.revision)?
    } else {
        (ObjectKey::new(&stamp.kind, &stamp.id), object.body)
    };
    Ok(Cached { stamp, key, body })
}

/// The sync identity and document of one stored `patched_fixture` record.
pub fn fixture_entry(body: Value, revision: u64) -> Answer<(ObjectKey, Value)> {
    let input = light_patch_wire::stored_fixture_input(body, revision)?;
    let key = ObjectKey::new(PATCHED_FIXTURE_KIND, input.fixture_id.to_string());
    let document = serde_json::to_value(input).map_err(|e| e.to_string())?;
    Ok((key, document))
}

/// The profile revision a fixture document is patched with.
pub fn fixture_profile(input: &Value) -> Option<ProfileKey> {
    let id = input.get("profile_id")?.as_str()?.parse().ok()?;
    let revision = input.get("profile_revision")?.as_u64()?;
    Some((id, revision))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
