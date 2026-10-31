//! Both copies created the same Group or Preset number for different things (TL-858).
//!
//! The show keeps its own object at the number. The copy's object moves to the next free
//! number, and every reference the copy made to it moves with it, so both survive the merge.
//! Presets are referenced by instance identity, so only their address changes; Groups are
//! referenced by key, so the copy's references are rewritten. Macro command text is never
//! rewritten: Macros that may name a moved number are listed for the operator instead.

use super::model::{MergeMacroWarning, MergeObjects, MergeRenumber};
use crate::show_compiler::migrations::identity::instance_identity;
use light_show::PortableShowObjectKey;
use serde_json::Value;
use std::collections::BTreeSet;

/// Moves the copy's colliding Groups and Presets to free numbers in `theirs`.
pub(super) fn separate_collisions(
    base: &MergeObjects,
    mine: &MergeObjects,
    theirs: &mut MergeObjects,
) -> Vec<MergeRenumber> {
    let collisions = theirs
        .iter()
        .filter(|(key, _)| matches!(key.kind(), "group" | "preset"))
        .filter(|(key, body)| {
            !base.contains_key(*key)
                && mine.get(*key).is_some_and(|existing| {
                    existing != *body
                        && (instance_identity(key.kind(), existing).is_none()
                            || instance_identity(key.kind(), existing)
                                != instance_identity(key.kind(), body))
                })
        })
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    let mut taken = base
        .keys()
        .chain(mine.keys())
        .chain(theirs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut renumbers = Vec::new();
    for from in collisions {
        let Some(to) = free_key(&from, &taken) else {
            continue;
        };
        taken.insert(to.clone());
        let mut body = theirs
            .remove(&from)
            .expect("collision key exists in theirs");
        readdress(&to, &mut body);
        let name = body.get("name").and_then(Value::as_str).map(str::to_owned);
        theirs.insert(to.clone(), body);
        if from.kind() == "group" {
            for body in theirs.values_mut() {
                rewrite_group_references(body, from.id(), to.id());
            }
        }
        renumbers.push(MergeRenumber { from, to, name });
    }
    renumbers
}

/// The next free number of the same kind (and Preset family) after the colliding one.
fn free_key(
    from: &PortableShowObjectKey,
    taken: &BTreeSet<PortableShowObjectKey>,
) -> Option<PortableShowObjectKey> {
    let (prefix, number) = match from.id().rsplit_once('.') {
        Some((family, number)) if from.kind() == "preset" => {
            (format!("{family}."), number.parse::<u64>().ok()?)
        }
        _ => match from.id().parse::<u64>() {
            Ok(number) => (String::new(), number),
            Err(_) => {
                return (2..)
                    .map(|suffix| {
                        PortableShowObjectKey::new(from.kind(), format!("{} {suffix}", from.id()))
                    })
                    .find(|key| !taken.contains(key));
            }
        },
    };
    (number + 1..)
        .map(|number| PortableShowObjectKey::new(from.kind(), format!("{prefix}{number}")))
        .find(|key| !taken.contains(key))
}

fn readdress(to: &PortableShowObjectKey, body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    match to.kind() {
        "group" => {
            object.insert("id".into(), Value::String(to.id().to_owned()));
        }
        "preset" => {
            if let Some(number) = to
                .id()
                .rsplit_once('.')
                .and_then(|(_, number)| number.parse::<u64>().ok())
            {
                object.insert("number".into(), Value::from(number));
            }
        }
        _ => {}
    }
}

/// Points every Group reference the copy made to `from` at `to`: Cue and Playback targets,
/// derived and referenced Group sources, and Group-owned Preset values.
fn rewrite_group_references(value: &mut Value, from: &str, to: &str) {
    match value {
        Value::Object(object) => {
            for field in ["group_id", "source_group_id"] {
                if object.get(field).and_then(Value::as_str) == Some(from) {
                    object.insert(field.into(), Value::String(to.to_owned()));
                }
            }
            if let Some(values) = object
                .get_mut("group_values")
                .and_then(Value::as_object_mut)
                && let Some(moved) = values.remove(from)
            {
                values.insert(to.to_owned(), moved);
            }
            for child in object.values_mut() {
                rewrite_group_references(child, from, to);
            }
        }
        Value::Array(elements) => {
            for element in elements {
                rewrite_group_references(element, from, to);
            }
        }
        _ => {}
    }
}

/// Macros whose command text names a number the merge moved.
pub(super) fn macro_warnings(
    macros: &MergeObjects,
    renumbers: &[MergeRenumber],
) -> Vec<MergeMacroWarning> {
    macros
        .iter()
        .filter(|(key, _)| key.kind() == "macro")
        .filter_map(|(key, body)| {
            let source = body.get("source").and_then(Value::as_str)?.to_lowercase();
            let mentions = renumbers
                .iter()
                .filter_map(|renumber| {
                    let token = match renumber.from.kind() {
                        "group" => format!("group {}", renumber.from.id()),
                        "preset" => format!("preset {}", renumber.from.id()),
                        _ => return None,
                    };
                    mentions(&source, &token.to_lowercase())
                        .then(|| format!("{} → {}", capitalize(&token), renumber.to.id()))
                })
                .collect::<Vec<_>>();
            (!mentions.is_empty()).then(|| MergeMacroWarning {
                key: key.clone(),
                name: body
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                mentions,
            })
        })
        .collect()
}

/// Whether `token` occurs in `text` as whole words, so "group 7" does not match "group 70".
fn mentions(text: &str, token: &str) -> bool {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.match_indices(token).any(|(index, _)| {
        let before = normalized[..index].chars().next_back();
        let after = normalized[index + token.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric)
            && !after.is_some_and(|character| character.is_alphanumeric() || character == '.')
    })
}

fn capitalize(text: &str) -> String {
    let mut characters = text.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_default()
}
