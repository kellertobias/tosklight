//! Portable standalone-Visualizer media layout objects.
//!
//! They are ordinary versioned show objects: backup, restore and selective import therefore use
//! the same portable history as every other authored object. Runtime source status and preview
//! frames deliberately do not appear here.

use light_show::{AtomicObjectDelete, AtomicObjectWrite, ShowStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const REQUEST_KIND: &str = "media_layout_request";
const MEDIA_KINDS: [&str; 5] = [
    "media_server",
    "media_source",
    "led_module_type",
    "media_surface",
    "media_projector",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitpConfiguration {
    #[serde(default = "default_citp_host")]
    pub host: String,
    #[serde(default = "default_citp_port")]
    pub port: u16,
    #[serde(default)]
    pub discovery_identity: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_citp_host() -> String {
    "127.0.0.1".to_owned()
}

const fn default_citp_port() -> u16 {
    4809
}

impl Default for CitpConfiguration {
    fn default() -> Self {
        Self {
            host: default_citp_host(),
            port: default_citp_port(),
            discovery_identity: None,
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaServer {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub citp: CitpConfiguration,
    #[serde(default)]
    pub last_known_endpoint: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSource {
    pub id: Uuid,
    pub server_id: Uuid,
    /// Numeric MSEX source identity is authoritative. Names and metadata only aid reconnection.
    pub advertised_source_id: u16,
    pub name: String,
    #[serde(default)]
    pub output_name: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub aspect_ratio: Option<f32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedModuleType {
    pub id: Uuid,
    pub name: String,
    pub width_metres: f32,
    pub height_metres: f32,
    pub pixel_pitch_millimetres: f32,
    #[serde(default)]
    pub horizontal_gap_metres: f32,
    #[serde(default)]
    pub vertical_gap_metres: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTransform {
    pub position_metres: [f32; 3],
    pub rotation_degrees: [f32; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRectangle {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for CropRectangle {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectionScreenMaterial {
    White,
    GreyHomeCinema,
    Custom {
        gain: f32,
        tint_srgb: String,
        roughness: f32,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaSurfaceSectionKind {
    ProjectionScreen {
        material: ProjectionScreenMaterial,
        #[serde(default)]
        edge_feather: f32,
    },
    Tv {
        #[serde(default)]
        bezel_metres: f32,
        #[serde(default)]
        spill: f32,
    },
    Led {
        module_type_id: Uuid,
        rows: u16,
        columns: u16,
        /// Row-major occupied cells. Missing indices are deliberate holes.
        occupied_cells: Vec<u32>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSurfaceSection {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub transform: MediaTransform,
    pub width_metres: f32,
    pub height_metres: f32,
    #[serde(default)]
    pub crop: CropRectangle,
    #[serde(flatten)]
    pub kind: MediaSurfaceSectionKind,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedFallbackReference {
    pub asset_id: Uuid,
    pub revision: u64,
    pub digest: String,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSurface {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub source_id: Option<Uuid>,
    #[serde(default)]
    pub fallback: Option<ManagedFallbackReference>,
    #[serde(default)]
    pub sections: Vec<MediaSurfaceSection>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProjector {
    pub id: Uuid,
    pub name: String,
    pub surface_id: Uuid,
    #[serde(default)]
    pub transform: MediaTransform,
    pub body_model: String,
    pub throw_ratio: f32,
    pub lens_shift: [f32; 2],
    pub cone_length_metres: f32,
    #[serde(default)]
    pub spill: f32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "body", rename_all = "snake_case")]
pub enum MediaObject {
    MediaServer(MediaServer),
    MediaSource(MediaSource),
    LedModuleType(LedModuleType),
    MediaSurface(MediaSurface),
    MediaProjector(MediaProjector),
}

impl MediaObject {
    fn kind(&self) -> &'static str {
        match self {
            Self::MediaServer(_) => "media_server",
            Self::MediaSource(_) => "media_source",
            Self::LedModuleType(_) => "led_module_type",
            Self::MediaSurface(_) => "media_surface",
            Self::MediaProjector(_) => "media_projector",
        }
    }

    fn id(&self) -> Uuid {
        match self {
            Self::MediaServer(value) => value.id,
            Self::MediaSource(value) => value.id,
            Self::LedModuleType(value) => value.id,
            Self::MediaSurface(value) => value.id,
            Self::MediaProjector(value) => value.id,
        }
    }

    fn body(&self) -> Result<Value, String> {
        match self {
            Self::MediaServer(value) => serde_json::to_value(value),
            Self::MediaSource(value) => serde_json::to_value(value),
            Self::LedModuleType(value) => serde_json::to_value(value),
            Self::MediaSurface(value) => serde_json::to_value(value),
            Self::MediaProjector(value) => serde_json::to_value(value),
        }
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedMediaObject {
    pub object: MediaObject,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaLayoutSnapshot {
    pub servers: Vec<VersionedMediaObject>,
    pub sources: Vec<VersionedMediaObject>,
    pub led_module_types: Vec<VersionedMediaObject>,
    pub surfaces: Vec<VersionedMediaObject>,
    pub projectors: Vec<VersionedMediaObject>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaIntentAction {
    Put { object: MediaObject },
    Delete { kind: String, id: Uuid },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaObjectIntent {
    pub request_id: String,
    pub expected_revision: u64,
    pub action: MediaIntentAction,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaLayoutOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub changed: bool,
    pub snapshot: MediaLayoutSnapshot,
}

fn read_kind<T>(
    store: &ShowStore,
    kind: &str,
    wrap: fn(T) -> MediaObject,
) -> Result<Vec<VersionedMediaObject>, String>
where
    T: for<'de> Deserialize<'de>,
{
    store
        .objects(kind)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|stored| {
            let parsed = serde_json::from_value(stored.body)
                .map_err(|error| format!("{kind} {} is malformed: {error}", stored.id))?;
            Ok(VersionedMediaObject {
                object: wrap(parsed),
                revision: stored.revision,
            })
        })
        .collect()
}

pub(crate) fn snapshot(store: &ShowStore) -> Result<MediaLayoutSnapshot, String> {
    Ok(MediaLayoutSnapshot {
        servers: read_kind(store, "media_server", MediaObject::MediaServer)?,
        sources: read_kind(store, "media_source", MediaObject::MediaSource)?,
        led_module_types: read_kind(store, "led_module_type", MediaObject::LedModuleType)?,
        surfaces: read_kind(store, "media_surface", MediaObject::MediaSurface)?,
        projectors: read_kind(store, "media_projector", MediaObject::MediaProjector)?,
    })
}

fn ids(entries: &[VersionedMediaObject]) -> BTreeSet<Uuid> {
    entries.iter().map(|entry| entry.object.id()).collect()
}

fn validate(layout: &MediaLayoutSnapshot) -> Result<(), String> {
    let servers = ids(&layout.servers);
    let sources = ids(&layout.sources);
    let modules = ids(&layout.led_module_types);
    let surfaces = ids(&layout.surfaces);
    for entry in &layout.sources {
        let MediaObject::MediaSource(source) = &entry.object else {
            unreachable!()
        };
        if !servers.contains(&source.server_id) {
            return Err(format!("media source {} references a missing server", source.name));
        }
        if source
            .aspect_ratio
            .is_some_and(|ratio| !ratio.is_finite() || ratio <= 0.0)
        {
            return Err(format!("media source {} has an invalid aspect ratio", source.name));
        }
    }
    for entry in &layout.led_module_types {
        let MediaObject::LedModuleType(module) = &entry.object else {
            unreachable!()
        };
        if module.width_metres <= 0.0
            || module.height_metres <= 0.0
            || module.pixel_pitch_millimetres <= 0.0
            || module.pixel_width == 0
            || module.pixel_height == 0
        {
            return Err(format!(
                "LED module type {} needs positive physical and pixel dimensions",
                module.name
            ));
        }
    }
    for entry in &layout.surfaces {
        let MediaObject::MediaSurface(surface) = &entry.object else {
            unreachable!()
        };
        if surface.source_id.is_some_and(|id| !sources.contains(&id)) {
            return Err(format!(
                "media surface {} references a missing source",
                surface.name
            ));
        }
        let mut section_ids = BTreeSet::new();
        for section in &surface.sections {
            if !section_ids.insert(section.id) {
                return Err(format!(
                    "media surface {} repeats a section identity",
                    surface.name
                ));
            }
            if section.width_metres <= 0.0 || section.height_metres <= 0.0 {
                return Err(format!("media section {} needs positive dimensions", section.name));
            }
            let crop = section.crop;
            if ![crop.left, crop.top, crop.width, crop.height]
                .into_iter()
                .all(f32::is_finite)
                || crop.left < 0.0
                || crop.top < 0.0
                || crop.width <= 0.0
                || crop.height <= 0.0
                || crop.left + crop.width > 1.0
                || crop.top + crop.height > 1.0
            {
                return Err(format!(
                    "media section {} crop must stay inside the source image",
                    section.name
                ));
            }
            if let MediaSurfaceSectionKind::Led {
                module_type_id,
                rows,
                columns,
                occupied_cells,
            } = &section.kind
            {
                if !modules.contains(module_type_id) || *rows == 0 || *columns == 0 {
                    return Err(format!(
                        "LED section {} has an invalid module layout",
                        section.name
                    ));
                }
                let cells = u32::from(*rows) * u32::from(*columns);
                if occupied_cells.iter().any(|cell| *cell >= cells) {
                    return Err(format!(
                        "LED section {} contains a cell outside its grid",
                        section.name
                    ));
                }
            }
        }
    }
    for entry in &layout.projectors {
        let MediaObject::MediaProjector(projector) = &entry.object else {
            unreachable!()
        };
        if !surfaces.contains(&projector.surface_id) {
            return Err(format!(
                "media projector {} references a missing surface",
                projector.name
            ));
        }
        if projector.throw_ratio <= 0.0 || projector.cone_length_metres <= 0.0 {
            return Err(format!(
                "media projector {} needs positive optics",
                projector.name
            ));
        }
    }
    Ok(())
}

pub(crate) fn apply(
    store: &ShowStore,
    intent: MediaObjectIntent,
) -> Result<MediaLayoutOutcome, String> {
    if intent.request_id.trim().is_empty() {
        return Err("media layout request identity is required".to_owned());
    }
    let request_id = intent.request_id.clone();
    let fingerprint = serde_json::to_string(&intent).map_err(|error| error.to_string())?;
    if let Some(previous) = store
        .objects(REQUEST_KIND)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.id == request_id)
    {
        if previous.body.get("fingerprint").and_then(Value::as_str) != Some(&fingerprint) {
            return Err("media layout request identity was already used for another intent".into());
        }
        return Ok(MediaLayoutOutcome {
            request_id,
            replayed: true,
            changed: false,
            snapshot: snapshot(store)?,
        });
    }

    let mut candidate = snapshot(store)?;
    let (kind, id, body, deleting) = match &intent.action {
        MediaIntentAction::Put { object } => (
            object.kind().to_owned(),
            object.id(),
            Some(object.body()?),
            false,
        ),
        MediaIntentAction::Delete { kind, id } => (kind.clone(), *id, None, true),
    };
    if !MEDIA_KINDS.contains(&kind.as_str()) {
        return Err(format!("unsupported media object kind {kind}"));
    }
    let existing = store
        .objects(&kind)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.id == id.to_string());
    let actual = existing.as_ref().map_or(0, |entry| entry.revision);
    if actual != intent.expected_revision {
        return Err(format!(
            "{kind} {id} revision conflict: expected {}, current {actual}",
            intent.expected_revision
        ));
    }

    let collection = match kind.as_str() {
        "media_server" => &mut candidate.servers,
        "media_source" => &mut candidate.sources,
        "led_module_type" => &mut candidate.led_module_types,
        "media_surface" => &mut candidate.surfaces,
        "media_projector" => &mut candidate.projectors,
        _ => unreachable!(),
    };
    collection.retain(|entry| entry.object.id() != id);
    if let Some(body) = &body {
        let object = match kind.as_str() {
            "media_server" => MediaObject::MediaServer(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
            "media_source" => MediaObject::MediaSource(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
            "led_module_type" => MediaObject::LedModuleType(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
            "media_surface" => MediaObject::MediaSurface(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
            "media_projector" => MediaObject::MediaProjector(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
            _ => unreachable!(),
        };
        collection.push(VersionedMediaObject {
            object,
            revision: actual.saturating_add(1),
        });
    }
    validate(&candidate)?;

    let ledger = serde_json::json!({ "fingerprint": fingerprint });
    let ledger_write = AtomicObjectWrite {
        kind: REQUEST_KIND,
        id: &request_id,
        body: &ledger,
        expected: 0,
    };
    let id_text = id.to_string();
    if deleting {
        let delete = AtomicObjectDelete {
            kind: &kind,
            id: &id_text,
            expected: actual,
        };
        store
            .mutate_objects_atomically(&[ledger_write], &[delete])
            .map_err(|error| error.to_string())?;
    } else {
        let body = body.as_ref().expect("put intent has a body");
        let write = AtomicObjectWrite {
            kind: &kind,
            id: &id_text,
            body,
            expected: actual,
        };
        store
            .mutate_objects_atomically(&[write, ledger_write], &[])
            .map_err(|error| error.to_string())?;
    }
    Ok(MediaLayoutOutcome {
        request_id,
        replayed: false,
        changed: true,
        snapshot: snapshot(store)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn show(name: &str) -> (PathBuf, ShowStore) {
        let root = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
            || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
            PathBuf::from,
        );
        let directory = root.join(format!("viz-media-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("media.show");
        let (store, _) = ShowStore::create(&path, "Media layout").unwrap();
        (path, store)
    }

    fn server(id: Uuid) -> MediaObject {
        MediaObject::MediaServer(MediaServer {
            id,
            name: "Playback rack".into(),
            citp: CitpConfiguration::default(),
            last_known_endpoint: None,
            extra: BTreeMap::from([("futureField".into(), Value::Bool(true))]),
        })
    }

    fn put(request: &str, object: MediaObject, expected_revision: u64) -> MediaObjectIntent {
        MediaObjectIntent {
            request_id: request.into(),
            expected_revision,
            action: MediaIntentAction::Put { object },
        }
    }

    #[test]
    fn an_old_show_opens_with_an_empty_additive_media_layout() {
        let (_path, store) = show("old");
        assert_eq!(snapshot(&store).unwrap(), MediaLayoutSnapshot::default());
    }

    #[test]
    fn typed_intents_are_revision_safe_replay_safe_and_preserve_unknown_fields() {
        let (path, store) = show("intent");
        let id = Uuid::new_v4();
        let intent = put("create-server", server(id), 0);
        let created = apply(&store, intent.clone()).unwrap();
        assert!(!created.replayed);
        assert_eq!(created.snapshot.servers[0].revision, 1);
        let replay = apply(&store, intent).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.snapshot.servers.len(), 1);
        assert!(apply(&store, put("stale", server(id), 0))
            .unwrap_err()
            .contains("revision conflict"));

        drop(store);
        let reopened = ShowStore::open(path).unwrap();
        let MediaObject::MediaServer(saved) = &snapshot(&reopened).unwrap().servers[0].object else {
            panic!("server")
        };
        assert_eq!(saved.extra["futureField"], Value::Bool(true));
    }

    #[test]
    fn references_allow_fan_out_and_block_unsafe_deletion() {
        let (_path, store) = show("references");
        let server_id = Uuid::new_v4();
        let source_id = Uuid::new_v4();
        apply(&store, put("server", server(server_id), 0)).unwrap();
        let source = MediaObject::MediaSource(MediaSource {
            id: source_id,
            server_id,
            advertised_source_id: 17,
            name: "Program".into(),
            output_name: Some("Output A".into()),
            width: Some(640),
            height: Some(360),
            aspect_ratio: Some(16.0 / 9.0),
            extra: BTreeMap::new(),
        });
        apply(&store, put("source", source, 0)).unwrap();
        for (index, name) in ["Main screen", "Confidence"].into_iter().enumerate() {
            let surface = MediaObject::MediaSurface(MediaSurface {
                id: Uuid::new_v4(),
                name: name.into(),
                source_id: Some(source_id),
                fallback: None,
                sections: vec![],
                extra: BTreeMap::new(),
            });
            apply(&store, put(&format!("surface-{index}"), surface, 0)).unwrap();
        }
        assert_eq!(snapshot(&store).unwrap().surfaces.len(), 2);
        let error = apply(
            &store,
            MediaObjectIntent {
                request_id: "delete-source".into(),
                expected_revision: 1,
                action: MediaIntentAction::Delete {
                    kind: "media_source".into(),
                    id: source_id,
                },
            },
        )
        .unwrap_err();
        assert!(error.contains("missing source"));
        assert_eq!(snapshot(&store).unwrap().sources.len(), 1);
    }

    #[test]
    fn invalid_crops_and_sparse_led_cells_are_rejected_without_partial_history() {
        let (_path, store) = show("validation");
        let module_id = Uuid::new_v4();
        let module = MediaObject::LedModuleType(LedModuleType {
            id: module_id,
            name: "500 mm panel".into(),
            width_metres: 0.5,
            height_metres: 0.5,
            pixel_pitch_millimetres: 3.9,
            horizontal_gap_metres: 0.005,
            vertical_gap_metres: 0.005,
            pixel_width: 128,
            pixel_height: 128,
            extra: BTreeMap::new(),
        });
        apply(&store, put("module", module, 0)).unwrap();
        let surface = MediaObject::MediaSurface(MediaSurface {
            id: Uuid::new_v4(),
            name: "Broken wall".into(),
            source_id: None,
            fallback: None,
            sections: vec![MediaSurfaceSection {
                id: Uuid::new_v4(),
                name: "Wall".into(),
                transform: MediaTransform::default(),
                width_metres: 2.0,
                height_metres: 1.0,
                crop: CropRectangle {
                    left: 0.75,
                    top: 0.0,
                    width: 0.5,
                    height: 1.0,
                },
                kind: MediaSurfaceSectionKind::Led {
                    module_type_id: module_id,
                    rows: 2,
                    columns: 4,
                    occupied_cells: vec![0, 1, 9],
                },
                extra: BTreeMap::new(),
            }],
            extra: BTreeMap::new(),
        });
        assert!(apply(&store, put("invalid-surface", surface, 0)).is_err());
        assert!(snapshot(&store).unwrap().surfaces.is_empty());
        let requests = store.objects(REQUEST_KIND).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, "module");
    }
}
