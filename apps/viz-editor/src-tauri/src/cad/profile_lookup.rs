//! What the plan reads out of a fixture's profile snapshot: the name of its mode, whether it is an
//! imported model, which fixtures are 3D Points, and the point a placement follows.
//!
//! The plan draws every placement where it was rigged: a point's live pose is desk state the plan
//! does not read, and moving the point changes nothing here. What the plan can do is name the
//! point a placement follows, the way the desk's Position Reference column names it.

use light_application::PatchSnapshot;
use std::collections::HashMap;
use uuid::Uuid;

/// Every 3D Point in the patch, by fixture id, named "ID · name".
///
/// A point is whatever carries the point position attribute in its patched mode — the same test
/// the desk applies — so the plan and the desk agree on what can be followed.
pub(super) fn position_points(snapshot: &PatchSnapshot) -> HashMap<Uuid, String> {
    let profiles: HashMap<_, _> = snapshot
        .profile_revisions
        .iter()
        .map(|profile| ((profile.profile_id.0, profile.profile_revision), profile))
        .collect();
    snapshot
        .fixtures
        .iter()
        .filter(|fixture| {
            profiles
                .get(&(
                    fixture.profile.profile_id.0,
                    fixture.profile.profile_revision,
                ))
                .is_some_and(|profile| {
                    mode_is_position_point(&profile.profile_snapshot, fixture.profile.mode_id)
                })
        })
        .map(|fixture| {
            let display_id = fixture
                .patch
                .fixture_number
                .map(|number| number.to_string())
                .or_else(|| {
                    fixture
                        .patch
                        .virtual_fixture_number
                        .map(|number| format!("0.{number}"))
                })
                .unwrap_or_else(|| "—".to_owned());
            (
                fixture.patch.fixture_id.0,
                format!("{display_id} · {}", fixture.patch.name),
            )
        })
        .collect()
}

fn mode_is_position_point(profile: &serde_json::Value, mode_id: Uuid) -> bool {
    let mode_id = mode_id.to_string();
    profile
        .get("modes")
        .and_then(serde_json::Value::as_array)
        .and_then(|modes| {
            modes
                .iter()
                .find(|mode| mode.get("id").and_then(serde_json::Value::as_str) == Some(&mode_id))
        })
        .and_then(|mode| mode.get("channels"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|channels| {
            channels.iter().any(|channel| {
                channel.get("attribute").and_then(serde_json::Value::as_str)
                    == Some("point.position.x")
            })
        })
}

/// The note for one placement: the name of the point its fixture follows, if the show holds it.
pub(super) fn position_reference(
    points: &HashMap<Uuid, String>,
    patch: &light_fixture::PatchedFixturePatch,
) -> Option<String> {
    patch
        .position_master
        .and_then(|master| points.get(&master).cloned())
}

/// The selected mode's name, for the drawing's label; a mode the profile no longer carries is said
/// so rather than guessed.
pub(super) fn mode_name(
    profile: Option<&&light_application::PatchProfileRevisionProjection>,
    mode_id: &str,
) -> String {
    profile
        .and_then(|profile| {
            profile
                .profile_snapshot
                .get("modes")?
                .as_array()?
                .iter()
                .find(|mode| mode.get("id").and_then(serde_json::Value::as_str) == Some(mode_id))
        })
        .and_then(|mode| mode.get("name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Unknown mode")
        .to_owned()
}

/// Whether a profile wraps a 3D model the operator imported into this show.
pub(super) fn imported_model(profile: &serde_json::Value) -> bool {
    profile
        .get("manufacturer")
        .and_then(serde_json::Value::as_str)
        == Some(crate::venue_models::IMPORTED_MANUFACTURER)
}
