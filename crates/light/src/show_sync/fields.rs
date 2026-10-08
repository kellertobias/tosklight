//! Per-field compare-and-set over JSON bodies addressed by RFC 6901 pointers.

use super::ShowSyncFieldEdit;
use crate::{ActionError, ActionErrorKind};
use serde_json::{Map, Value};

/// What one field edit did to a body.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum FieldResolution {
    /// The field still held the client's base and now holds its value.
    Applied,
    /// The field already holds the requested value, so a retry or a convergent edit is a no-op.
    Unchanged,
    /// Someone else changed the field; it keeps `theirs`.
    Conflict { theirs: Option<Value> },
}

/// Applies each edit whose base still matches, in order, and reports every field's resolution.
pub(super) fn apply_field_edits(
    body: &mut Value,
    edits: &[ShowSyncFieldEdit],
) -> Result<Vec<FieldResolution>, ActionError> {
    edits
        .iter()
        .map(|edit| apply_field_edit(body, edit))
        .collect()
}

fn apply_field_edit(
    body: &mut Value,
    edit: &ShowSyncFieldEdit,
) -> Result<FieldResolution, ActionError> {
    let tokens = parse_pointer(&edit.path)?;
    let current = present(lookup(body, &tokens));
    let wanted = present(edit.value.as_ref());
    if current == wanted {
        return Ok(FieldResolution::Unchanged);
    }
    if current != present(edit.base.as_ref()) {
        return Ok(FieldResolution::Conflict {
            theirs: current.cloned(),
        });
    }
    match wanted {
        Some(value) => assign(body, &tokens, value.clone())?,
        None => remove(body, &tokens)?,
    }
    Ok(FieldResolution::Applied)
}

/// JSON `null` and an absent field are the same state for compare-and-set.
pub(super) fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|value| !value.is_null())
}

pub(super) fn parse_pointer(path: &str) -> Result<Vec<String>, ActionError> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(invalid(format!(
            "field path {path:?} must be a JSON pointer starting with '/'"
        )));
    };
    Ok(rest
        .split('/')
        .map(|token| token.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn lookup<'a>(body: &'a Value, tokens: &[String]) -> Option<&'a Value> {
    tokens.iter().try_fold(body, |value, token| match value {
        Value::Object(map) => map.get(token),
        Value::Array(items) => token
            .parse::<usize>()
            .ok()
            .and_then(|index| items.get(index)),
        _ => None,
    })
}

fn assign(body: &mut Value, tokens: &[String], value: Value) -> Result<(), ActionError> {
    let Some((last, parents)) = tokens.split_last() else {
        if !value.is_object() {
            return Err(invalid("a whole object body must be a JSON object"));
        }
        *body = value;
        return Ok(());
    };
    let mut parent = body;
    for token in parents {
        parent = child_for_write(parent, token)?;
    }
    match parent {
        Value::Object(map) => {
            map.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(items) => {
            let index = array_index(last, items.len())?;
            if index == items.len() {
                items.push(value);
            } else {
                items[index] = value;
            }
            Ok(())
        }
        _ => Err(invalid(format!(
            "field parent of {last:?} is not a container"
        ))),
    }
}

/// Descends one level, creating an absent object level so a field can be added below it.
fn child_for_write<'a>(value: &'a mut Value, token: &str) -> Result<&'a mut Value, ActionError> {
    if value.is_null() {
        *value = Value::Object(Map::new());
    }
    match value {
        Value::Object(map) => Ok(map
            .entry(token.to_owned())
            .or_insert_with(|| Value::Object(Map::new()))),
        Value::Array(items) => {
            let length = items.len();
            token
                .parse::<usize>()
                .ok()
                .and_then(|index| items.get_mut(index))
                .ok_or_else(|| {
                    invalid(format!(
                        "array index {token:?} is outside an array of {length}"
                    ))
                })
        }
        _ => Err(invalid(format!(
            "field parent {token:?} is not a container"
        ))),
    }
}

fn remove(body: &mut Value, tokens: &[String]) -> Result<(), ActionError> {
    let Some((last, parents)) = tokens.split_last() else {
        return Err(invalid(
            "a whole object body cannot be removed by a field edit",
        ));
    };
    let Some(parent) = parents
        .iter()
        .try_fold(&mut *body, |value, token| match value {
            Value::Object(map) => map.get_mut(token),
            Value::Array(items) => token
                .parse::<usize>()
                .ok()
                .and_then(move |index| items.get_mut(index)),
            _ => None,
        })
    else {
        return Ok(());
    };
    match parent {
        Value::Object(map) => {
            map.remove(last);
        }
        Value::Array(items) => {
            if let Ok(index) = last.parse::<usize>()
                && index < items.len()
            {
                items.remove(index);
            }
        }
        _ => {}
    }
    Ok(())
}

fn array_index(token: &str, maximum: usize) -> Result<usize, ActionError> {
    token
        .parse::<usize>()
        .ok()
        .filter(|index| *index <= maximum)
        .ok_or_else(|| invalid(format!("array index {token:?} is outside the array")))
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
