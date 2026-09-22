//! Which parts of the plan an operator can currently see and touch.
//!
//! Locking and hiding are planning decisions, not show data: a hidden layer or a hidden fixture
//! stays patched and stays in the show, it simply drops out of this view and out of reach of the
//! pointer. Keeping those questions together means the plan view and the selection logic always
//! agree on what is on the sheet.

use super::Session;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

/// The layers an operator has locked, so a fixture parked on one of them cannot be
/// picked up or dragged by accident while the rest of the plan stays workable.
pub(super) fn locked_layers(session: &Session) -> Result<BTreeSet<String>, String> {
    session.with(|document| {
        document
            .objects("patch_layer")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        object
                            .body
                            .get("locked")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                    })
                    .map(|object| object.id)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

/// The layers switched off for the given view, named by the layer property the view
/// reads. A layer counts as shown unless it says otherwise, so a plan drawn before
/// the property existed keeps showing everything.
pub(super) fn hidden_layers(session: &Session, property: &str) -> Result<BTreeSet<String>, String> {
    session.with(|document| {
        document
            .objects("patch_layer")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        !object
                            .body
                            .get(property)
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true)
                    })
                    .map(|object| object.id)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

/// The individual fixtures an operator has hidden in the given view, independently of
/// the layer they sit on. Like a layer, a fixture is shown unless it says otherwise.
pub(super) fn hidden_fixture_ids(
    session: &Session,
    property: &str,
) -> Result<BTreeSet<Uuid>, String> {
    session.with(|document| {
        document
            .objects("fixture_visibility")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter(|object| {
                        !object
                            .body
                            .get(property)
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true)
                    })
                    .filter_map(|object| {
                        object
                            .body
                            .get("fixtureId")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| Uuid::parse_str(id).ok())
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

/// The note an operator has written on a fixture, keyed by fixture, so the plan can
/// print it beside the lamp it belongs to.
pub(super) fn fixture_notes(session: &Session) -> Result<HashMap<Uuid, String>, String> {
    session.with(|document| {
        document
            .objects("fixture_note")
            .map(|objects| {
                objects
                    .into_iter()
                    .filter_map(|object| {
                        let fixture_id = object
                            .body
                            .get("fixtureId")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| Uuid::parse_str(id).ok())?;
                        let note = object
                            .body
                            .get("note")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        Some((fixture_id, note))
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

/// The fixtures an operator can actually grab in the plan view: everything patched,
/// less whatever a locked layer, a hidden layer or a hidden fixture takes out of reach.
pub(super) fn selectable_ids(session: &Session) -> Result<BTreeSet<Uuid>, String> {
    let locked = locked_layers(session)?;
    let hidden_fixtures = hidden_fixture_ids(session, "visible2d")?;
    let hidden_layers = hidden_layers(session, "visible2d")?;
    session.with(|document| {
        document
            .patch_snapshot()
            .map(|snapshot| {
                snapshot
                    .fixtures
                    .into_iter()
                    .filter(|fixture| {
                        !locked.contains(&fixture.patch.layer_id)
                            && !hidden_layers.contains(&fixture.patch.layer_id)
                            && !hidden_fixtures.contains(&fixture.patch.fixture_id.0)
                    })
                    .map(|fixture| fixture.patch.fixture_id.0)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}
