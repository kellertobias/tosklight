use crate::{FixtureProfileRevision, ShowStore};
use light_core::FixtureId;
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

pub(super) const PROFILE_A: &str = "00000000-0000-0000-0000-000000000001";
pub(super) const PROFILE_Z: &str = "00000000-0000-0000-0000-0000000000ff";
pub(super) const PROFILE_PORTABLE: &str = "00000000-0000-0000-0000-000000000009";

pub(super) fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-profile-{name}-{}.sqlite", Uuid::new_v4()))
}

pub(super) fn create(name: &str) -> (PathBuf, ShowStore) {
    let path = temporary(name);
    let (store, _) = ShowStore::create(&path, "Profile test").unwrap();
    (path, store)
}

pub(super) fn profile_value(id: &str, revision: u64, name: &str) -> Value {
    json!({
        "schema_version": 2,
        "id": id,
        "revision": revision,
        "manufacturer": "Acme",
        "name": name,
        "modes": [],
        "photograph_asset": "data:image/png;base64,iVBORw0KGgo=",
        "model_asset": "data:model/gltf-binary;base64,Z2xURg==",
        "future_asset_manifest": {
            "checksum": "retain-exactly",
            "files": ["photo.png", "model.glb"]
        }
    })
}

pub(super) fn profile(id: &str, revision: u64, name: &str) -> FixtureProfileRevision {
    FixtureProfileRevision::from_profile(profile_value(id, revision, name)).unwrap()
}

pub(super) fn fixture_id(id: &str) -> FixtureId {
    FixtureId(Uuid::parse_str(id).unwrap())
}
