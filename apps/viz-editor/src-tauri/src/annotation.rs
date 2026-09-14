//! Lines, boxes, text and measurements the operator draws on a CAD view.
//!
//! They are show data, like placed venue drawings: they travel with the document through save-as,
//! backup and transfer, and a show written before they existed simply carries none. Each one
//! belongs to the CAD view it was drawn on, and its points are plan millimetres on that view before
//! a top-down tile's quarter-turn rotation, so turning a tile turns the drawing with the rig.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use uuid::Uuid;

use crate::session::Session;

/// The stored-object kind a drawn item lives in.
const KIND: &str = "cad_annotation";

/// Broadcast when an item is drawn, changed or erased, so every window redraws.
const ANNOTATION_DELTA_EVENT: &str = "cad-annotation-delta";

/// The CAD views an item can belong to, as the frontend names them.
const VIEWS: [&str; 5] = [
    "top_down",
    "left_to_right",
    "right_to_left",
    "front_to_back",
    "back_to_front",
];

type Answer<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CadAnnotationKind {
    /// A run of straight segments, open or closed.
    Polyline,
    /// A rectangle given by two opposite corners.
    Box,
    /// A line of text anchored at one point.
    Text,
    /// A dimension between two points, labelled with their distance.
    Measure,
}

/// One drawn item, as the show stores it and the CAD screen draws it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CadAnnotation {
    /// Empty when the frontend asks for a new item; the store assigns one.
    #[serde(default)]
    pub id: String,
    pub view: String,
    pub kind: CadAnnotationKind,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub text: String,
    /// How tall text reads on the plan, in millimetres.
    #[serde(default = "default_text_height")]
    pub text_height_millimetres: f64,
}

fn default_text_height() -> f64 {
    250.0
}

/// Refuse an item the CAD screen could not draw, with the reason.
pub fn validate(annotation: &CadAnnotation) -> Answer<()> {
    if !VIEWS.contains(&annotation.view.as_str()) {
        return Err(format!("{} is not a CAD view", annotation.view));
    }
    if annotation
        .points
        .iter()
        .flatten()
        .any(|value| !value.is_finite())
    {
        return Err("a drawn item's points must be finite millimetres".to_owned());
    }
    let count = annotation.points.len();
    match annotation.kind {
        CadAnnotationKind::Polyline if count < 2 => {
            Err("a line needs at least two points".to_owned())
        }
        CadAnnotationKind::Box | CadAnnotationKind::Measure if count != 2 => {
            Err("a box or a measurement needs exactly two points".to_owned())
        }
        CadAnnotationKind::Text if count != 1 => Err("text needs exactly one point".to_owned()),
        CadAnnotationKind::Text if annotation.text.trim().is_empty() => {
            Err("text needs something to say".to_owned())
        }
        CadAnnotationKind::Text
            if !annotation.text_height_millimetres.is_finite()
                || annotation.text_height_millimetres <= 0.0 =>
        {
            Err("text needs a height greater than zero".to_owned())
        }
        _ => Ok(()),
    }
}

fn store(session: &Session, annotation: &CadAnnotation) -> Answer<()> {
    let body = serde_json::to_value(annotation).map_err(|error| error.to_string())?;
    session.change(|document| {
        document
            .put_object(KIND, &annotation.id, &body)
            .map_err(|error| error.to_string())
    })
}

fn announce(app: &tauri::AppHandle, session: &Session) -> Answer<()> {
    let annotations = read_annotations(session)?;
    app.emit(ANNOTATION_DELTA_EVENT, annotations)
        .map_err(|error| error.to_string())
}

fn read_annotations(session: &Session) -> Answer<Vec<CadAnnotation>> {
    session.with(|document| {
        let stored = document.objects(KIND).map_err(|error| error.to_string())?;
        Ok(stored
            .into_iter()
            .filter_map(|object| serde_json::from_value::<CadAnnotation>(object.body).ok())
            .collect())
    })
}

/// The drawn items the open show carries.
#[tauri::command]
pub fn cad_annotations(session: tauri::State<'_, Session>) -> Answer<Vec<CadAnnotation>> {
    read_annotations(&session)
}

/// Store one drawn item, assigning an ID to a new one.
#[tauri::command]
pub fn save_cad_annotation(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    mut annotation: CadAnnotation,
) -> Answer<CadAnnotation> {
    validate(&annotation)?;
    if annotation.id.trim().is_empty() {
        annotation.id = Uuid::new_v4().to_string();
    }
    store(&session, &annotation)?;
    announce(&app, &session)?;
    Ok(annotation)
}

/// Erase one drawn item from the show.
#[tauri::command]
pub fn delete_cad_annotation(
    app: tauri::AppHandle,
    session: tauri::State<'_, Session>,
    id: String,
) -> Answer<()> {
    session.change(|document| {
        document
            .delete_object(KIND, &id)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })?;
    announce(&app, &session)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: CadAnnotationKind, points: Vec<[f64; 2]>) -> CadAnnotation {
        CadAnnotation {
            id: String::new(),
            view: "top_down".to_owned(),
            kind,
            points,
            closed: false,
            text: String::new(),
            text_height_millimetres: 250.0,
        }
    }

    #[test]
    fn accepts_every_kind_with_the_points_it_is_drawn_from() {
        assert!(
            validate(&item(
                CadAnnotationKind::Polyline,
                vec![[0.0, 0.0], [1.0, 2.0]]
            ))
            .is_ok()
        );
        assert!(validate(&item(CadAnnotationKind::Box, vec![[0.0, 0.0], [1.0, 2.0]])).is_ok());
        assert!(
            validate(&item(
                CadAnnotationKind::Measure,
                vec![[0.0, 0.0], [1.0, 2.0]]
            ))
            .is_ok()
        );
        let mut text = item(CadAnnotationKind::Text, vec![[0.0, 0.0]]);
        text.text = "FOH".to_owned();
        assert!(validate(&text).is_ok());
    }

    #[test]
    fn refuses_items_that_cannot_be_drawn() {
        assert!(validate(&item(CadAnnotationKind::Polyline, vec![[0.0, 0.0]])).is_err());
        assert!(validate(&item(CadAnnotationKind::Box, vec![[0.0, 0.0]])).is_err());
        assert!(validate(&item(CadAnnotationKind::Text, vec![[0.0, 0.0]])).is_err());
        assert!(
            validate(&item(
                CadAnnotationKind::Measure,
                vec![[0.0, f64::NAN], [1.0, 1.0]]
            ))
            .is_err()
        );
        let mut elsewhere = item(CadAnnotationKind::Box, vec![[0.0, 0.0], [1.0, 1.0]]);
        elsewhere.view = "isometric".to_owned();
        assert!(validate(&elsewhere).is_err());
    }

    #[test]
    fn reads_a_stored_body_that_omits_the_optional_fields() {
        let stored = serde_json::json!({
            "id": "a",
            "view": "front_to_back",
            "kind": "measure",
            "points": [[0.0, 0.0], [3000.0, 0.0]],
        });
        let annotation: CadAnnotation = serde_json::from_value(stored).unwrap();
        assert_eq!(annotation.kind, CadAnnotationKind::Measure);
        assert!(!annotation.closed);
        assert_eq!(annotation.text_height_millimetres, 250.0);
    }
}
