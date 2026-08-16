//! Portable standalone-Visualizer media layout objects.
//!
//! They are ordinary versioned show objects: backup, restore and selective import therefore use
//! the same portable history as every other authored object. Runtime source status and preview
//! frames deliberately do not appear here.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use light_show::{AtomicObjectDelete, AtomicObjectWrite, ShowStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const REQUEST_KIND: &str = "media_layout_request";
const MEDIA_KINDS: [&str; 6] = [
    "media_fallback_asset",
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
        #[serde(default, alias = "edgeFeather")]
        edge_feather: f32,
    },
    Tv {
        #[serde(default)]
        bezel_metres: f32,
        #[serde(default)]
        spill: f32,
    },
    Led {
        #[serde(alias = "moduleTypeId")]
        module_type_id: Uuid,
        rows: u16,
        columns: u16,
        /// Row-major occupied cells. Missing indices are deliberate holes.
        #[serde(alias = "occupiedCells")]
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

/// Immutable fallback bytes owned by the portable show itself.
///
/// The encoded bytes are intentionally an ordinary typed show object: SQLite backup/restore and
/// selective import already preserve object history atomically, without a lossy sidecar path.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFallbackAsset {
    pub id: Uuid,
    pub name: String,
    pub media_type: String,
    pub digest: String,
    pub width: u32,
    pub height: u32,
    pub bytes_base64: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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
    MediaFallbackAsset(MediaFallbackAsset),
    MediaServer(MediaServer),
    MediaSource(MediaSource),
    LedModuleType(LedModuleType),
    MediaSurface(MediaSurface),
    MediaProjector(MediaProjector),
}

impl MediaObject {
    fn kind(&self) -> &'static str {
        match self {
            Self::MediaFallbackAsset(_) => "media_fallback_asset",
            Self::MediaServer(_) => "media_server",
            Self::MediaSource(_) => "media_source",
            Self::LedModuleType(_) => "led_module_type",
            Self::MediaSurface(_) => "media_surface",
            Self::MediaProjector(_) => "media_projector",
        }
    }

    fn id(&self) -> Uuid {
        match self {
            Self::MediaFallbackAsset(value) => value.id,
            Self::MediaServer(value) => value.id,
            Self::MediaSource(value) => value.id,
            Self::LedModuleType(value) => value.id,
            Self::MediaSurface(value) => value.id,
            Self::MediaProjector(value) => value.id,
        }
    }

    fn body(&self) -> Result<Value, String> {
        match self {
            Self::MediaFallbackAsset(value) => serde_json::to_value(value),
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
    #[serde(default)]
    pub fallback_assets: Vec<VersionedMediaObject>,
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
        fallback_assets: read_kind(
            store,
            "media_fallback_asset",
            MediaObject::MediaFallbackAsset,
        )?,
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
    let fallback_assets = ids(&layout.fallback_assets);
    let servers = ids(&layout.servers);
    let sources = ids(&layout.sources);
    let modules = ids(&layout.led_module_types);
    let surfaces = ids(&layout.surfaces);
    for entry in &layout.fallback_assets {
        let MediaObject::MediaFallbackAsset(asset) = &entry.object else {
            unreachable!()
        };
        let bytes = BASE64.decode(&asset.bytes_base64).map_err(|error| {
            format!("fallback asset {} is not valid base64: {error}", asset.name)
        })?;
        let digest = hex_digest(Sha256::digest(&bytes));
        if bytes.is_empty() || digest != asset.digest {
            return Err(format!(
                "fallback asset {} bytes do not match its digest",
                asset.name
            ));
        }
        let decoded = image::load_from_memory(&bytes)
            .map_err(|error| format!("fallback asset {} cannot be decoded: {error}", asset.name))?;
        if decoded.width() != asset.width || decoded.height() != asset.height {
            return Err(format!(
                "fallback asset {} dimensions do not match its bytes",
                asset.name
            ));
        }
    }
    for entry in &layout.sources {
        let MediaObject::MediaSource(source) = &entry.object else {
            unreachable!()
        };
        if !servers.contains(&source.server_id) {
            return Err(format!(
                "media source {} references a missing server",
                source.name
            ));
        }
        if source
            .aspect_ratio
            .is_some_and(|ratio| !ratio.is_finite() || ratio <= 0.0)
        {
            return Err(format!(
                "media source {} has an invalid aspect ratio",
                source.name
            ));
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
        if let Some(fallback) = &surface.fallback {
            if !fallback_assets.contains(&fallback.asset_id) {
                return Err(format!(
                    "media surface {} references a missing fallback asset",
                    surface.name
                ));
            }
            let asset = layout
                .fallback_assets
                .iter()
                .find(|entry| entry.object.id() == fallback.asset_id)
                .expect("set and collection agree");
            let MediaObject::MediaFallbackAsset(asset_body) = &asset.object else {
                unreachable!()
            };
            if asset.revision != fallback.revision
                || asset_body.digest != fallback.digest
                || asset_body.media_type != fallback.media_type
                || asset_body.width != fallback.width
                || asset_body.height != fallback.height
            {
                return Err(format!(
                    "media surface {} fallback metadata is stale or inconsistent",
                    surface.name
                ));
            }
        }
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
                return Err(format!(
                    "media section {} needs positive dimensions",
                    section.name
                ));
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

pub(crate) fn import_fallback(
    store: &ShowStore,
    request_id: String,
    expected_revision: u64,
    name: String,
    bytes: &[u8],
) -> Result<(ManagedFallbackReference, MediaLayoutOutcome), String> {
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err("fallback image must contain at most 32 MiB".into());
    }
    let format = image::guess_format(bytes)
        .map_err(|error| format!("fallback image format is not recognized: {error}"))?;
    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err("fallback image must be PNG, JPEG or WebP".into()),
    };
    let decoded = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| format!("fallback image cannot be decoded: {error}"))?;
    let id = Uuid::new_v4();
    let digest = hex_digest(Sha256::digest(bytes));
    let asset = MediaFallbackAsset {
        id,
        name,
        media_type: media_type.into(),
        digest: digest.clone(),
        width: decoded.width(),
        height: decoded.height(),
        bytes_base64: BASE64.encode(bytes),
        extra: BTreeMap::new(),
    };
    let outcome = apply(
        store,
        MediaObjectIntent {
            request_id,
            expected_revision,
            action: MediaIntentAction::Put {
                object: MediaObject::MediaFallbackAsset(asset),
            },
        },
    )?;
    let revision = outcome
        .snapshot
        .fallback_assets
        .iter()
        .find(|entry| entry.object.id() == id)
        .map(|entry| entry.revision)
        .ok_or_else(|| "imported fallback did not appear in the committed layout".to_owned())?;
    Ok((
        ManagedFallbackReference {
            asset_id: id,
            revision,
            digest,
            media_type: media_type.into(),
            width: decoded.width(),
            height: decoded.height(),
        },
        outcome,
    ))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
        "media_fallback_asset" => &mut candidate.fallback_assets,
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
            "media_fallback_asset" => MediaObject::MediaFallbackAsset(
                serde_json::from_value(body.clone()).map_err(|error| error.to_string())?,
            ),
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
        assert!(
            apply(&store, put("stale", server(id), 0))
                .unwrap_err()
                .contains("revision conflict")
        );

        drop(store);
        let reopened = ShowStore::open(path).unwrap();
        let MediaObject::MediaServer(saved) = &snapshot(&reopened).unwrap().servers[0].object
        else {
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
    fn fallback_bytes_survive_backup_and_are_dependency_guarded() {
        let (path, store) = show("fallback");
        let png = BASE64
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        let (fallback, imported) = import_fallback(
            &store,
            "fallback-import".into(),
            0,
            "Standby.png".into(),
            &png,
        )
        .unwrap();
        assert_eq!(fallback.media_type, "image/png");
        assert_eq!((fallback.width, fallback.height), (1, 1));
        assert_eq!(imported.snapshot.fallback_assets.len(), 1);

        let surface_id = Uuid::new_v4();
        apply(
            &store,
            put(
                "fallback-surface",
                MediaObject::MediaSurface(MediaSurface {
                    id: surface_id,
                    name: "Fallback only".into(),
                    source_id: None,
                    fallback: Some(fallback.clone()),
                    sections: vec![],
                    extra: BTreeMap::new(),
                }),
                0,
            ),
        )
        .unwrap();

        let backup = path.with_file_name("backup.show");
        store.backup_to(&backup).unwrap();
        let restored = ShowStore::open(backup).unwrap();
        let restored_layout = snapshot(&restored).unwrap();
        assert_eq!(
            restored_layout.fallback_assets,
            snapshot(&store).unwrap().fallback_assets
        );
        assert_eq!(restored_layout.surfaces.len(), 1);

        let error = apply(
            &store,
            MediaObjectIntent {
                request_id: "delete-used-fallback".into(),
                expected_revision: fallback.revision,
                action: MediaIntentAction::Delete {
                    kind: "media_fallback_asset".into(),
                    id: fallback.asset_id,
                },
            },
        )
        .unwrap_err();
        assert!(error.contains("missing fallback asset"));
        assert_eq!(snapshot(&store).unwrap().fallback_assets.len(), 1);
    }

    #[test]
    fn malformed_fallbacks_do_not_create_history() {
        let (_path, store) = show("bad-fallback");
        assert!(
            import_fallback(
                &store,
                "bad-fallback".into(),
                0,
                "broken.png".into(),
                b"not an image",
            )
            .is_err()
        );
        assert!(snapshot(&store).unwrap().fallback_assets.is_empty());
        assert!(store.objects(REQUEST_KIND).unwrap().is_empty());
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

    #[test]
    fn media_sections_accept_legacy_camel_case_and_keep_screens_module_free() {
        let module_id = Uuid::new_v4();
        let led: MediaSurfaceSection = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "name": "Legacy LED wall",
            "widthMetres": 2.0,
            "heightMetres": 1.0,
            "type": "led",
            "moduleTypeId": module_id,
            "rows": 2,
            "columns": 4,
            "occupiedCells": [0, 1, 2, 3]
        }))
        .expect("the shipped camel-case demo remains readable");
        assert!(matches!(
            led.kind,
            MediaSurfaceSectionKind::Led { module_type_id, .. } if module_type_id == module_id
        ));

        let screen: MediaSurfaceSection = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "name": "Projection screen",
            "widthMetres": 4.0,
            "heightMetres": 2.25,
            "type": "projection_screen",
            "material": { "type": "white" },
            "edgeFeather": 0.02
        }))
        .expect("a projection screen never needs an LED module");
        assert!(matches!(
            screen.kind,
            MediaSurfaceSectionKind::ProjectionScreen { edge_feather, .. }
                if (edge_feather - 0.02).abs() < f32::EPSILON
        ));
    }
}
