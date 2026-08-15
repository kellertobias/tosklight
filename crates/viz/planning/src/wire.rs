//! What the visualizer reads from a scene source.
//!
//! The renderer already knows how to consume a lighting desk, and a planning document describes
//! the same rig, so it is served in the same shape rather than given a second protocol to learn.
//! Only the fields the renderer actually reads are projected here — this is the visualizer's view
//! of a document, not a general-purpose API.

use light_application::{PatchProfileRevisionProjection, PatchSnapshot};
use light_fixture::{
    InstalledFixtureAppearance, MultiPatchInstance, PatchedFixturePatch, SplitPatch,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SelectionSnapshot {
    pub revision: u64,
    pub selected_fixture_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct Readiness {
    pub status: &'static str,
    pub active_show: Option<Uuid>,
    pub active_show_error: Option<String>,
    pub snapshot_revision: u64,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub token: String,
    /// The renderer checks this and warns when a source cannot promise read-only. A planning
    /// document has no other client and no output, so the promise is trivially true.
    pub role: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PatchSnapshotDto {
    pub show_id: Uuid,
    pub show_revision: u64,
    pub patch_revision: u64,
    pub fixtures: Vec<FixtureDto>,
    pub profile_revisions: Vec<ProfileRevisionDto>,
}

#[derive(Debug, Serialize)]
pub struct FixtureDto {
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub name: String,
    pub profile_id: Uuid,
    pub profile_revision: u64,
    pub mode_id: Uuid,
    pub split_patches: Vec<SplitDto>,
    pub location: LocationDto,
    pub rotation: RotationDto,
    pub multipatch: Vec<MultiPatchDto>,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    pub bracket_angle: f32,
    pub shaper_angle: Option<f32>,
    pub installed_appearance: InstalledFixtureAppearance,
}

#[derive(Debug, Serialize)]
pub struct SplitDto {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

#[derive(Debug, Default, Serialize)]
pub struct LocationDto {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Default, Serialize)]
pub struct RotationDto {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Serialize)]
pub struct MultiPatchDto {
    pub id: Uuid,
    pub name: String,
    pub split_patches: Vec<SplitDto>,
    pub location: LocationDto,
    pub rotation: RotationDto,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    pub bracket_angle: f32,
    pub shaper_angle: Option<f32>,
    pub installed_appearance: InstalledFixtureAppearance,
}

#[derive(Debug, Serialize)]
pub struct ProfileRevisionDto {
    pub profile_id: Uuid,
    pub profile_revision: u64,
    pub manufacturer: String,
    pub name: String,
    pub fixture_type: String,
    pub patch_policy: String,
    /// The renderer decodes DMX from this, so it never needs the fixture library.
    pub profile_snapshot: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ObjectCollection {
    pub objects: Vec<ObjectRecord>,
}

#[derive(Debug, Serialize)]
pub struct ObjectRecord {
    pub id: String,
    pub revision: u64,
    pub body: serde_json::Value,
}

pub fn patch_snapshot(snapshot: PatchSnapshot) -> PatchSnapshotDto {
    PatchSnapshotDto {
        show_id: snapshot.show_id.0,
        show_revision: snapshot.show_revision.value(),
        patch_revision: snapshot.patch_revision.value(),
        fixtures: snapshot
            .fixtures
            .into_iter()
            .map(|projection| fixture(&projection.patch, &projection.profile))
            .collect(),
        profile_revisions: snapshot
            .profile_revisions
            .into_iter()
            .map(profile_revision)
            .collect(),
    }
}

fn fixture(
    patch: &PatchedFixturePatch,
    profile: &light_fixture::PatchedFixtureProfileReference,
) -> FixtureDto {
    FixtureDto {
        fixture_id: patch.fixture_id.0,
        fixture_number: patch.fixture_number,
        name: patch.name.clone(),
        profile_id: profile.profile_id.0,
        profile_revision: profile.profile_revision,
        mode_id: profile.mode_id,
        split_patches: patch.split_patches.iter().map(split).collect(),
        location: LocationDto {
            x: patch.location.x,
            y: patch.location.y,
            z: patch.location.z,
        },
        rotation: RotationDto {
            x: patch.rotation.x,
            y: patch.rotation.y,
            z: patch.rotation.z,
        },
        multipatch: patch.multipatch.iter().map(multipatch).collect(),
        invert_pan: patch.invert_pan,
        invert_tilt: patch.invert_tilt,
        bracket_angle: patch.bracket_angle,
        shaper_angle: patch.shaper_angle,
        installed_appearance: patch.installed_appearance.clone(),
    }
}

fn multipatch(instance: &MultiPatchInstance) -> MultiPatchDto {
    MultiPatchDto {
        id: instance.id,
        name: instance.name.clone(),
        split_patches: instance.split_patches.iter().map(split).collect(),
        location: LocationDto {
            x: instance.location.x,
            y: instance.location.y,
            z: instance.location.z,
        },
        rotation: RotationDto {
            x: instance.rotation.x,
            y: instance.rotation.y,
            z: instance.rotation.z,
        },
        invert_pan: instance.invert_pan,
        invert_tilt: instance.invert_tilt,
        bracket_angle: instance.bracket_angle,
        shaper_angle: instance.shaper_angle,
        installed_appearance: instance.installed_appearance.clone(),
    }
}

fn split(patch: &SplitPatch) -> SplitDto {
    SplitDto {
        split: patch.split,
        universe: patch.universe,
        address: patch.address,
    }
}

fn profile_revision(profile: PatchProfileRevisionProjection) -> ProfileRevisionDto {
    ProfileRevisionDto {
        profile_id: profile.profile_id.0,
        profile_revision: profile.profile_revision,
        manufacturer: profile.manufacturer,
        name: profile.name,
        fixture_type: profile.fixture_type,
        patch_policy: match profile.patch_policy {
            light_fixture::PatchPolicy::Dmx => "dmx".to_owned(),
            light_fixture::PatchPolicy::VisualOnly => "visual_only".to_owned(),
            light_fixture::PatchPolicy::Internal => "internal".to_owned(),
        },
        profile_snapshot: profile.profile_snapshot,
    }
}
