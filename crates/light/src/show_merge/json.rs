//! Structural three-way merge of one JSON body.
//!
//! Maps merge key by key and lists of identified elements (Cues by `id`, Cue changes by fixture
//! and attribute, …) merge element by element, so edits to different Cues, or to different
//! fixtures in one Cue, never conflict. Anything else that both sides changed differently is a
//! conflict, decided by a recorded resolution or left for the operator.

use super::model::{MergeChoice, MergeConflict, MergeResolutions};
use light_show::PortableShowObjectKey;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// Fields that identify an element of a list, tried in order. A list merges element by element
/// only when every element on every side has a unique identity under one of them.
const ELEMENT_IDENTITIES: &[&[&str]] = &[
    &["id"],
    &["fixture_id", "attribute"],
    &["group_id", "attribute"],
    &["fixture_id"],
    &["group_id"],
];

pub(super) struct BodyMerger<'a> {
    key: &'a PortableShowObjectKey,
    resolutions: &'a MergeResolutions,
    pub(super) conflicts: Vec<MergeConflict>,
}

#[derive(Clone)]
struct Location {
    id: String,
    path: Vec<String>,
}

impl Location {
    fn child(&self, id: &str, label: String) -> Self {
        let mut path = self.path.clone();
        path.push(label);
        Self {
            id: format!("{}/{id}", self.id),
            path,
        }
    }
}

impl<'a> BodyMerger<'a> {
    pub(super) fn new(key: &'a PortableShowObjectKey, resolutions: &'a MergeResolutions) -> Self {
        Self {
            key,
            resolutions,
            conflicts: Vec::new(),
        }
    }

    /// The merged body, or `None` when the merged object is deleted.
    pub(super) fn merge(
        &mut self,
        base: Option<&Value>,
        mine: Option<&Value>,
        theirs: Option<&Value>,
    ) -> Option<Value> {
        let root = Location {
            id: format!("{}/{}", self.key.kind(), self.key.id()),
            path: Vec::new(),
        };
        self.merge_at(&root, base, mine, theirs)
    }

    fn merge_at(
        &mut self,
        at: &Location,
        base: Option<&Value>,
        mine: Option<&Value>,
        theirs: Option<&Value>,
    ) -> Option<Value> {
        if same(mine, theirs) || same(base, theirs) {
            return mine.cloned();
        }
        if same(base, mine) {
            return theirs.cloned();
        }
        match (base, mine, theirs) {
            (None | Some(Value::Object(_)), Some(Value::Object(m)), Some(Value::Object(t))) => {
                let empty = Map::new();
                let b = base.and_then(Value::as_object).unwrap_or(&empty);
                Some(Value::Object(self.merge_maps(at, b, m, t)))
            }
            (None | Some(Value::Array(_)), Some(Value::Array(m)), Some(Value::Array(t))) => {
                let b = base
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice);
                match element_identity(b, m, t) {
                    Some(fields) => Some(Value::Array(self.merge_lists(at, fields, b, m, t))),
                    None => self.conflict(at, base, mine, theirs),
                }
            }
            _ => self.conflict(at, base, mine, theirs),
        }
    }

    fn merge_maps(
        &mut self,
        at: &Location,
        base: &Map<String, Value>,
        mine: &Map<String, Value>,
        theirs: &Map<String, Value>,
    ) -> Map<String, Value> {
        let mut keys = mine.keys().cloned().collect::<Vec<_>>();
        keys.extend(
            theirs
                .keys()
                .filter(|key| !mine.contains_key(*key))
                .cloned(),
        );
        keys.extend(
            base.keys()
                .filter(|key| !mine.contains_key(*key) && !theirs.contains_key(*key))
                .cloned(),
        );
        let mut merged = Map::new();
        for key in keys {
            let child = at.child(&escape(&key), key.clone());
            if let Some(value) =
                self.merge_at(&child, base.get(&key), mine.get(&key), theirs.get(&key))
            {
                merged.insert(key, value);
            }
        }
        merged
    }

    fn merge_lists(
        &mut self,
        at: &Location,
        fields: &[&str],
        base: &[Value],
        mine: &[Value],
        theirs: &[Value],
    ) -> Vec<Value> {
        fn find<'v>(list: &'v [Value], fields: &[&str], id: &str) -> Option<&'v Value> {
            list.iter()
                .find(|element| identity(element, fields).as_deref() == Some(id))
        }
        // Mine's order wins; an element only theirs has follows its predecessor in theirs.
        let mut order = mine
            .iter()
            .filter_map(|element| identity(element, fields))
            .collect::<Vec<_>>();
        let mut previous: Option<String> = None;
        for id in theirs
            .iter()
            .filter_map(|element| identity(element, fields))
        {
            if !order.contains(&id) {
                let position = previous
                    .as_ref()
                    .and_then(|previous| order.iter().position(|id| id == previous))
                    .map_or(0, |index| index + 1);
                order.insert(position, id.clone());
            }
            previous = Some(id);
        }
        order.extend(
            base.iter()
                .filter_map(|element| identity(element, fields))
                .filter(|id| !order.contains(id))
                .collect::<Vec<_>>(),
        );
        let mut merged = Vec::with_capacity(order.len());
        for id in order {
            let (b, m, t) = (
                find(base, fields, &id),
                find(mine, fields, &id),
                find(theirs, fields, &id),
            );
            let cue = at.path.last().is_some_and(|parent| parent == "cues");
            let label = element_label(m.or(t).or(b), &id, cue);
            let child = at.child(&format!("[{}]", escape(&id)), label);
            if let Some(value) = self.merge_at(&child, b, m, t) {
                merged.push(value);
            }
        }
        merged
    }

    fn conflict(
        &mut self,
        at: &Location,
        base: Option<&Value>,
        mine: Option<&Value>,
        theirs: Option<&Value>,
    ) -> Option<Value> {
        match self.resolutions.get(&at.id) {
            Some(MergeChoice::Mine) => mine.cloned(),
            Some(MergeChoice::Theirs) => theirs.cloned(),
            None => {
                self.conflicts.push(MergeConflict {
                    id: at.id.clone(),
                    key: self.key.clone(),
                    path: at.path.clone(),
                    base: base.cloned(),
                    mine: mine.cloned(),
                    theirs: theirs.cloned(),
                });
                // Until the operator decides, the show keeps what it has.
                mine.cloned()
            }
        }
    }
}

fn same(left: Option<&Value>, right: Option<&Value>) -> bool {
    left == right
}

fn element_identity(
    base: &[Value],
    mine: &[Value],
    theirs: &[Value],
) -> Option<&'static [&'static str]> {
    ELEMENT_IDENTITIES.iter().copied().find(|fields| {
        [base, mine, theirs].iter().all(|list| {
            let mut seen = BTreeSet::new();
            list.iter().all(|element| {
                identity(element, fields).is_some_and(|identity| seen.insert(identity))
            })
        })
    })
}

fn identity(element: &Value, fields: &[&str]) -> Option<String> {
    let object = element.as_object()?;
    let parts = fields
        .iter()
        .map(|field| match object.get(*field)? {
            Value::String(value) => Some(value.clone()),
            Value::Null => None,
            value => Some(value.to_string()),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(parts.join("|"))
}

/// How an operator recognises a list element: a Cue by its number, anything else by its name.
fn element_label(element: Option<&Value>, identity: &str, cue: bool) -> String {
    let field = |name: &str| {
        element
            .and_then(|element| element.get(name))
            .and_then(|value| match value {
                Value::String(text) if !text.is_empty() => Some(text.clone()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
    };
    cue.then(|| field("number"))
        .flatten()
        .map(|number| format!("Cue {number}"))
        .or_else(|| field("name"))
        .unwrap_or_else(|| identity.to_owned())
}

fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
