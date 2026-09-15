//! How the operator arranges the CAD drawings: folders, and where each drawing sits among them.
//!
//! The arrangement is one object of its own rather than a field on every drawing. Placed venue
//! drawings and drawn items keep exactly the records they had, a show written before folders
//! existed reads as every drawing at the top level, and a drawing the arrangement does not mention
//! yet — just placed, or placed in another window — simply shows at the top level too.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use tauri::Emitter;

use crate::session::Session;

/// The stored-object kind the arrangement lives in, and the one object of it a show carries.
const KIND: &str = "cad_drawing_tree";
const ID: &str = "tree";

/// Broadcast when the arrangement changes, so every window's tree follows.
const DRAWING_TREE_DELTA_EVENT: &str = "cad-drawing-tree-delta";

type Answer<T> = Result<T, String>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrawingFolder {
    pub id: String,
    pub name: String,
    /// The folder this one sits in; none is the top level.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Position among its siblings, folders and drawings alike.
    #[serde(default)]
    pub order: f64,
    #[serde(default)]
    pub collapsed: bool,
}

/// Where one drawing — a placed venue drawing or a drawn item, by its ID — sits in the tree.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrawingPlacement {
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub order: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTree {
    #[serde(default)]
    pub folders: Vec<DrawingFolder>,
    #[serde(default)]
    pub items: BTreeMap<String, DrawingPlacement>,
}

/// Refuse an arrangement the tree could not draw: unnamed or repeated folders, a parent that does
/// not exist, or a folder inside itself.
pub fn validate(tree: &DrawingTree) -> Answer<()> {
    let mut ids = HashSet::new();
    for folder in &tree.folders {
        if folder.id.trim().is_empty() {
            return Err("a folder needs an ID".to_owned());
        }
        if folder.name.trim().is_empty() {
            return Err("a folder needs a name".to_owned());
        }
        if !ids.insert(folder.id.as_str()) {
            return Err(format!("the folder {} appears twice", folder.name));
        }
    }
    let known = |parent: &Option<String>| parent.as_deref().is_none_or(|id| ids.contains(id));
    if let Some(folder) = tree.folders.iter().find(|folder| !known(&folder.parent_id)) {
        return Err(format!(
            "the folder {} is in a folder that does not exist",
            folder.name
        ));
    }
    if tree.items.values().any(|item| !known(&item.parent_id)) {
        return Err("a drawing is in a folder that does not exist".to_owned());
    }
    let parent_of = |id: &str| {
        tree.folders
            .iter()
            .find(|folder| folder.id == id)
            .and_then(|folder| folder.parent_id.as_deref())
    };
    for folder in &tree.folders {
        let mut current = folder.parent_id.as_deref();
        for _ in 0..=tree.folders.len() {
            match current {
                None => break,
                Some(id) if id == folder.id => {
                    return Err(format!(
                        "the folder {} cannot be inside itself",
                        folder.name
                    ));
                }
                Some(id) => current = parent_of(id),
            }
        }
    }
    Ok(())
}

fn read_tree(session: &Session) -> Answer<DrawingTree> {
    session.with(|document| {
        let stored = document.objects(KIND).map_err(|error| error.to_string())?;
        Ok(stored
            .into_iter()
            .find_map(|object| serde_json::from_value::<DrawingTree>(object.body).ok())
            .unwrap_or_default())
    })
}

/// The arrangement of the open show's drawings.
#[tauri::command]
pub fn cad_drawing_tree(session: tauri::State<'_, Session>) -> Answer<DrawingTree> {
    read_tree(&session)
}

/// Store the whole arrangement at once, so a move is one write that cannot land half done.
#[tauri::command]
pub fn save_cad_drawing_tree(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    tree: DrawingTree,
) -> Answer<DrawingTree> {
    validate(&tree)?;
    let body = serde_json::to_value(&tree).map_err(|error| error.to_string())?;
    session.change(|document| {
        document
            .put_object(KIND, ID, &body)
            .map_err(|error| error.to_string())
    })?;
    app.emit(DRAWING_TREE_DELTA_EVENT, &tree)
        .map_err(|error| error.to_string())?;
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: &str, parent: Option<&str>) -> DrawingFolder {
        DrawingFolder {
            id: id.to_owned(),
            name: id.to_uppercase(),
            parent_id: parent.map(str::to_owned),
            order: 0.0,
            collapsed: false,
        }
    }

    #[test]
    fn accepts_nested_folders_and_drawings_placed_in_them() {
        let mut tree = DrawingTree {
            folders: vec![folder("venue", None), folder("floor", Some("venue"))],
            ..DrawingTree::default()
        };
        tree.items.insert(
            "plan".to_owned(),
            DrawingPlacement {
                parent_id: Some("floor".to_owned()),
                order: 1.0,
            },
        );
        assert!(validate(&tree).is_ok());
    }

    #[test]
    fn refuses_a_missing_parent_a_repeated_folder_and_a_folder_inside_itself() {
        let orphan = DrawingTree {
            folders: vec![folder("floor", Some("gone"))],
            ..DrawingTree::default()
        };
        assert!(validate(&orphan).is_err());
        let twice = DrawingTree {
            folders: vec![folder("a", None), folder("a", None)],
            ..DrawingTree::default()
        };
        assert!(validate(&twice).is_err());
        let cycle = DrawingTree {
            folders: vec![folder("a", Some("b")), folder("b", Some("a"))],
            ..DrawingTree::default()
        };
        assert!(validate(&cycle).is_err());
        let mut lost = DrawingTree::default();
        lost.items.insert(
            "plan".to_owned(),
            DrawingPlacement {
                parent_id: Some("gone".to_owned()),
                order: 0.0,
            },
        );
        assert!(validate(&lost).is_err());
    }

    #[test]
    fn reads_an_empty_body_as_every_drawing_at_the_top_level() {
        let tree: DrawingTree = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(tree, DrawingTree::default());
    }
}
