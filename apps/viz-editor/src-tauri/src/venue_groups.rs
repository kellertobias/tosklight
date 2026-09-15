//! Groups of Venue elements the operator put together in the CAD, such as the trusses of one rig.
//!
//! A group is a name and the fixture IDs of its members, kept as one object of its own beside the
//! patch rather than as a field on every placement, so a show written before groups existed reads
//! as having none and the patch records stay exactly as they were. A member deleted from the show
//! is dropped from its group when the groups are read, and a group left with no member goes with it.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use tauri::Emitter;
use uuid::Uuid;

use crate::session::Session;

/// The stored-object kind the groups live in, and the one object of it a show carries.
const KIND: &str = "cad_venue_groups";
const ID: &str = "groups";

/// Broadcast when the groups change, so every window's Elements panel and selection follow.
const VENUE_GROUPS_DELTA_EVENT: &str = "cad-venue-groups-delta";

type Answer<T> = Result<T, String>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VenueGroup {
    pub id: String,
    pub name: String,
    /// The grouped placements' fixture IDs, in the order they were grouped.
    #[serde(default)]
    pub member_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VenueGroups {
    #[serde(default)]
    pub groups: Vec<VenueGroup>,
}

/// Refuse groups the panel could not show: unnamed or repeated groups, a group with no member, or
/// one element in two groups.
pub fn validate(groups: &VenueGroups) -> Answer<()> {
    let mut ids = HashSet::new();
    let mut members = HashSet::new();
    for group in &groups.groups {
        if group.id.trim().is_empty() {
            return Err("a group needs an ID".to_owned());
        }
        if group.name.trim().is_empty() {
            return Err("a group needs a name".to_owned());
        }
        if !ids.insert(group.id.as_str()) {
            return Err(format!("the group {} appears twice", group.name));
        }
        if group.member_ids.is_empty() {
            return Err(format!("the group {} has no elements", group.name));
        }
        if let Some(member) = group.member_ids.iter().find(|id| !members.insert(**id)) {
            return Err(format!("the element {member} is in more than one group"));
        }
    }
    Ok(())
}

/// The groups with every member the show no longer has left out, and empty groups dropped.
pub fn prune(groups: VenueGroups, known: &BTreeSet<Uuid>) -> VenueGroups {
    VenueGroups {
        groups: groups
            .groups
            .into_iter()
            .map(|mut group| {
                group.member_ids.retain(|id| known.contains(id));
                group
            })
            .filter(|group| !group.member_ids.is_empty())
            .collect(),
    }
}

fn fixture_ids(session: &Session) -> Answer<BTreeSet<Uuid>> {
    session.with(|document| {
        document
            .patch_snapshot()
            .map(|snapshot| {
                snapshot
                    .fixtures
                    .into_iter()
                    .map(|fixture| fixture.patch.fixture_id.0)
                    .collect()
            })
            .map_err(|error| error.to_string())
    })
}

pub fn read_groups(session: &Session) -> Answer<VenueGroups> {
    let stored = session.with(|document| {
        let objects = document.objects(KIND).map_err(|error| error.to_string())?;
        Ok(objects
            .into_iter()
            .find_map(|object| serde_json::from_value::<VenueGroups>(object.body).ok())
            .unwrap_or_default())
    })?;
    Ok(prune(stored, &fixture_ids(session)?))
}

pub fn store_groups(session: &Session, groups: VenueGroups) -> Answer<VenueGroups> {
    validate(&groups)?;
    let known = fixture_ids(session)?;
    if let Some(missing) = groups
        .groups
        .iter()
        .flat_map(|group| &group.member_ids)
        .find(|id| !known.contains(id))
    {
        return Err(format!("the element {missing} is not in the show"));
    }
    let body = serde_json::to_value(&groups).map_err(|error| error.to_string())?;
    session.change(|document| {
        document
            .put_object(KIND, ID, &body)
            .map_err(|error| error.to_string())
    })?;
    Ok(groups)
}

/// The open show's Venue element groups.
#[tauri::command]
pub fn cad_venue_groups(session: tauri::State<'_, Session>) -> Answer<VenueGroups> {
    read_groups(&session)
}

/// Store every group at once, so grouping or ungrouping is one write that cannot land half done.
#[tauri::command]
pub fn save_cad_venue_groups(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    groups: VenueGroups,
) -> Answer<VenueGroups> {
    let stored = store_groups(&session, groups)?;
    app.emit(VENUE_GROUPS_DELTA_EVENT, &stored)
        .map_err(|error| error.to_string())?;
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_document::PlanningDocument;

    fn group(id: &str, members: &[Uuid]) -> VenueGroup {
        VenueGroup {
            id: id.to_owned(),
            name: id.to_uppercase(),
            member_ids: members.to_vec(),
        }
    }

    #[test]
    fn accepts_separate_groups_and_refuses_what_the_panel_could_not_show() {
        let [a, b, c] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let fine = VenueGroups {
            groups: vec![group("rig", &[a, b]), group("deck", &[c])],
        };
        assert!(validate(&fine).is_ok());
        let twice = VenueGroups {
            groups: vec![group("rig", &[a]), group("rig", &[b])],
        };
        assert!(validate(&twice).is_err());
        let shared = VenueGroups {
            groups: vec![group("rig", &[a, b]), group("deck", &[b])],
        };
        assert!(
            validate(&shared)
                .unwrap_err()
                .contains("more than one group")
        );
        let empty = VenueGroups {
            groups: vec![group("rig", &[])],
        };
        assert!(validate(&empty).is_err());
        let mut unnamed = group("rig", &[a]);
        unnamed.name = " ".into();
        assert!(
            validate(&VenueGroups {
                groups: vec![unnamed]
            })
            .is_err()
        );
    }

    #[test]
    fn a_deleted_member_leaves_its_group_and_an_emptied_group_goes() {
        let [a, b, c] = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let pruned = prune(
            VenueGroups {
                groups: vec![group("rig", &[a, b]), group("deck", &[c])],
            },
            &BTreeSet::from([a]),
        );
        assert_eq!(pruned.groups, vec![group("rig", &[a])]);
    }

    #[test]
    fn reads_a_show_without_groups_as_none() {
        let groups: VenueGroups = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(groups, VenueGroups::default());
    }

    #[test]
    fn stored_groups_are_read_back_without_members_the_show_does_not_have() {
        let path = std::env::temp_dir().join(format!("cad-venue-groups-{}.show", Uuid::new_v4()));
        drop(PlanningDocument::create(&path, "Venue groups test").unwrap());
        let session = Session::default();
        session.open(&path).unwrap();
        let gone = Uuid::new_v4();
        assert!(
            store_groups(
                &session,
                VenueGroups {
                    groups: vec![group("rig", &[gone])]
                }
            )
            .unwrap_err()
            .contains("not in the show")
        );
        // A group written while its element still existed reads as empty once it is deleted.
        session
            .change(|document| {
                document
                    .put_object(
                        KIND,
                        ID,
                        &serde_json::to_value(VenueGroups {
                            groups: vec![group("rig", &[gone])],
                        })
                        .unwrap(),
                    )
                    .map_err(|error| error.to_string())
            })
            .unwrap();
        assert_eq!(read_groups(&session).unwrap(), VenueGroups::default());
        assert_eq!(
            store_groups(&session, VenueGroups::default()).unwrap(),
            VenueGroups::default()
        );
        let _ = std::fs::remove_file(path);
    }
}
