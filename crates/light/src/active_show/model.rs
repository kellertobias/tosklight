use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{AttributeConfiguration, Revision, ShowId};
use light_dynamics::DynamicDefinition;
use light_output::OutputRoute;
use light_playback::{CueList, PlaybackDefinition, PlaybackPage};
use light_programmer::{GroupDefinition, Preset};
use light_show::{LosslessBody, PortableJson, PortableShowRevision};
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap};

/// Portable show-object families whose runtime semantics are owned by the active-show boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActiveShowObjectKind {
    AttributeConfiguration,
    CueList,
    Dynamic,
    Group,
    Macro,
    PatchLayer,
    Playback,
    PlaybackPage,
    Preset,
    /// Live tracking: which tracker is which 3D Point, and what a zone does.
    Psn,
    Schedule,
    StageLayout,
    Timecode,
    UserLayout,
}

/// One decoded active-show body. The enum is the application contract: a mutation cannot pair a
/// known storage kind with another family's body. Exact extensible JSON remains encapsulated by
/// `light-show::LosslessBody`.
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveShowObjectBody {
    AttributeConfiguration(LosslessBody<AttributeConfiguration>),
    CueList(LosslessBody<CueList>),
    Dynamic(LosslessBody<DynamicDefinition>),
    Group(LosslessBody<GroupDefinition>),
    Macro(LosslessBody<crate::CommandMacroDefinition>),
    PatchLayer(LosslessBody<PatchLayer>),
    Playback(LosslessBody<PlaybackDefinition>),
    PlaybackPage(LosslessBody<PlaybackPage>),
    Preset(LosslessBody<Preset>),
    Psn(LosslessBody<super::PsnConfiguration>),
    Schedule(LosslessBody<crate::ScheduleDefinition>),
    StageLayout(LosslessBody<StageLayout>),
    Timecode(LosslessBody<light_playback::TimecodeDefinition>),
    UserLayout(LosslessBody<UserLayout>),
}

impl ActiveShowObjectBody {
    pub fn decode(kind: ActiveShowObjectKind, raw: serde_json::Value) -> serde_json::Result<Self> {
        validate_family_shape(kind, &raw)?;
        Ok(match kind {
            ActiveShowObjectKind::AttributeConfiguration => {
                Self::AttributeConfiguration(LosslessBody::decode(raw)?)
            }
            ActiveShowObjectKind::CueList => Self::CueList(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Dynamic => Self::Dynamic(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Group => Self::Group(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Macro => Self::Macro(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::PatchLayer => Self::PatchLayer(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Playback => Self::Playback(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::PlaybackPage => Self::PlaybackPage(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Preset => Self::Preset(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Psn => Self::Psn(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Schedule => Self::Schedule(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::StageLayout => Self::StageLayout(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Timecode => Self::Timecode(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::UserLayout => Self::UserLayout(LosslessBody::decode(raw)?),
        })
    }

    pub const fn kind(&self) -> ActiveShowObjectKind {
        match self {
            Self::AttributeConfiguration(_) => ActiveShowObjectKind::AttributeConfiguration,
            Self::CueList(_) => ActiveShowObjectKind::CueList,
            Self::Dynamic(_) => ActiveShowObjectKind::Dynamic,
            Self::Group(_) => ActiveShowObjectKind::Group,
            Self::Macro(_) => ActiveShowObjectKind::Macro,
            Self::PatchLayer(_) => ActiveShowObjectKind::PatchLayer,
            Self::Playback(_) => ActiveShowObjectKind::Playback,
            Self::PlaybackPage(_) => ActiveShowObjectKind::PlaybackPage,
            Self::Preset(_) => ActiveShowObjectKind::Preset,
            Self::Psn(_) => ActiveShowObjectKind::Psn,
            Self::Schedule(_) => ActiveShowObjectKind::Schedule,
            Self::StageLayout(_) => ActiveShowObjectKind::StageLayout,
            Self::Timecode(_) => ActiveShowObjectKind::Timecode,
            Self::UserLayout(_) => ActiveShowObjectKind::UserLayout,
        }
    }

    pub fn encode(&self) -> serde_json::Value {
        match self {
            Self::AttributeConfiguration(body) => body.encode(),
            Self::CueList(body) => body.encode(),
            Self::Dynamic(body) => body.encode(),
            Self::Group(body) => body.encode(),
            Self::Macro(body) => body.encode(),
            Self::PatchLayer(body) => body.encode(),
            Self::Playback(body) => body.encode(),
            Self::PlaybackPage(body) => body.encode(),
            Self::Preset(body) => body.encode(),
            Self::Psn(body) => body.encode(),
            Self::Schedule(body) => body.encode(),
            Self::StageLayout(body) => body.encode(),
            Self::Timecode(body) => body.encode(),
            Self::UserLayout(body) => body.encode(),
        }
    }

    pub(crate) fn attribute_configuration(&self) -> Option<&LosslessBody<AttributeConfiguration>> {
        match self {
            Self::AttributeConfiguration(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn cue_list(&self) -> Option<&LosslessBody<CueList>> {
        match self {
            Self::CueList(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn dynamic(&self) -> Option<&LosslessBody<DynamicDefinition>> {
        match self {
            Self::Dynamic(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn group(&self) -> Option<&LosslessBody<GroupDefinition>> {
        match self {
            Self::Group(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn macro_definition(&self) -> Option<&LosslessBody<crate::CommandMacroDefinition>> {
        match self {
            Self::Macro(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn patch_layer(&self) -> Option<&LosslessBody<PatchLayer>> {
        match self {
            Self::PatchLayer(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn playback(&self) -> Option<&LosslessBody<PlaybackDefinition>> {
        match self {
            Self::Playback(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn playback_page(&self) -> Option<&LosslessBody<PlaybackPage>> {
        match self {
            Self::PlaybackPage(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn preset(&self) -> Option<&LosslessBody<Preset>> {
        match self {
            Self::Preset(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn schedule(&self) -> Option<&LosslessBody<crate::ScheduleDefinition>> {
        match self {
            Self::Schedule(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn psn(&self) -> Option<&LosslessBody<super::PsnConfiguration>> {
        match self {
            Self::Psn(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn stage_layout(&self) -> Option<&LosslessBody<StageLayout>> {
        match self {
            Self::StageLayout(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn user_layout(&self) -> Option<&LosslessBody<UserLayout>> {
        match self {
            Self::UserLayout(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn timecode(&self) -> Option<&LosslessBody<light_playback::TimecodeDefinition>> {
        match self {
            Self::Timecode(body) => Some(body),
            _ => None,
        }
    }
}

fn validate_family_shape(
    kind: ActiveShowObjectKind,
    raw: &serde_json::Value,
) -> serde_json::Result<()> {
    let object = raw.as_object().ok_or_else(|| {
        <serde_json::Error as serde::de::Error>::custom(format!(
            "{} body must be an object",
            kind.as_str()
        ))
    })?;
    if kind == ActiveShowObjectKind::UserLayout {
        let has_desks = object.contains_key("desks");
        let has_active_desk = object.contains_key("activeDeskId");
        if has_desks && has_active_desk {
            return Ok(());
        }
        if !has_desks && !has_active_desk && !looks_like_other_family(object) {
            // Early show files permitted opaque per-user layout payloads. They remain readable and
            // lossless, while bodies recognizable as another supported family are still rejected.
            return Ok(());
        }
    }
    if kind == ActiveShowObjectKind::StageLayout {
        if object.is_empty()
            || ["positions", "positions3d", "camera3d", "positions2dConfig"]
                .iter()
                .any(|field| object.contains_key(*field))
        {
            return Ok(());
        }
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "stage_layout body has no Stage Layout fields",
        ));
    }
    if kind == ActiveShowObjectKind::Psn {
        // Every field of a tracking configuration has a default, so an empty body is the valid
        // "off, nothing bound" one. A body carrying another family's fields is still refused.
        if object.is_empty()
            || ["enabled", "bindings", "zones", "calibration", "group"]
                .iter()
                .any(|field| object.contains_key(*field))
        {
            return Ok(());
        }
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "psn body has no tracking fields",
        ));
    }
    if kind == ActiveShowObjectKind::Group {
        return validate_group_family_shape(object);
    }
    let required: &[&str] = match kind {
        ActiveShowObjectKind::AttributeConfiguration => &[
            "version",
            "custom_attributes",
            "placements",
            "activation_groups",
        ],
        ActiveShowObjectKind::CueList => &["id", "name", "cues"],
        ActiveShowObjectKind::Dynamic => &[
            "id",
            "pool_number",
            "revision",
            "name",
            "target_binding",
            "lanes",
            "phase",
            "speed",
            "default_activation",
        ],
        ActiveShowObjectKind::Group => unreachable!("handled above"),
        ActiveShowObjectKind::Macro => &["id", "number", "name", "source"],
        ActiveShowObjectKind::PatchLayer => &["id", "name", "order"],
        ActiveShowObjectKind::Playback => &["number", "name", "target"],
        ActiveShowObjectKind::PlaybackPage => &["number", "name", "slots", "virtual_playbacks"],
        // `values` was absent from early empty Presets and is defaulted by the typed model.
        ActiveShowObjectKind::Preset => &["name", "family", "number"],
        ActiveShowObjectKind::Psn => unreachable!("handled above"),
        ActiveShowObjectKind::Schedule => &["id", "name", "enabled", "trigger", "target"],
        ActiveShowObjectKind::StageLayout => unreachable!("handled above"),
        ActiveShowObjectKind::Timecode => &[
            "id",
            "number",
            "name",
            "transport_offset",
            "auto_start",
            "markers",
            "lanes",
        ],
        ActiveShowObjectKind::UserLayout => &["desks", "activeDeskId"],
    };
    if let Some(field) = required.iter().find(|field| !object.contains_key(**field)) {
        return Err(<serde_json::Error as serde::de::Error>::custom(format!(
            "{} body is missing required field {field}",
            kind.as_str()
        )));
    }
    Ok(())
}

fn validate_group_family_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Result<()> {
    if !object.contains_key("name") {
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "group body is missing required field name",
        ));
    }

    let Some(source) = object.get("source").filter(|source| !source.is_null()) else {
        if object.contains_key("fixtures") {
            return Ok(());
        }
        return Err(<serde_json::Error as serde::de::Error>::custom(
            "group body requires fixtures or a canonical source",
        ));
    };
    let source = source.as_object().ok_or_else(|| {
        <serde_json::Error as serde::de::Error>::custom("group source must be an object")
    })?;
    let source_type = source
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            <serde_json::Error as serde::de::Error>::custom(
                "group source is missing required string field type",
            )
        })?;
    let required_array = match source_type {
        "explicit" => "fixture_ids",
        "references" => "references",
        other => {
            return Err(<serde_json::Error as serde::de::Error>::custom(format!(
                "group source has unsupported type {other}"
            )));
        }
    };
    if !source
        .get(required_array)
        .is_some_and(serde_json::Value::is_array)
    {
        return Err(<serde_json::Error as serde::de::Error>::custom(format!(
            "group {source_type} source requires array field {required_array}"
        )));
    }
    Ok(())
}

fn looks_like_other_family(object: &serde_json::Map<String, serde_json::Value>) -> bool {
    [
        &[
            "version",
            "custom_attributes",
            "placements",
            "activation_groups",
        ][..],
        &["id", "name", "cues"][..],
        &["id", "number", "name", "source"],
        &["id", "pool_number", "revision", "target_binding", "lanes"],
        &[
            "id",
            "number",
            "name",
            "transport_offset",
            "auto_start",
            "markers",
            "lanes",
        ],
        &["name", "order"],
        &["number", "name", "target"],
        &["number", "name", "slots", "virtual_playbacks"],
        &["name", "family", "number"],
        &["id", "name", "enabled", "trigger", "target"],
        &["positions"],
    ]
    .into_iter()
    .any(|required| required.iter().all(|field| object.contains_key(*field)))
        || (object.contains_key("name")
            && (object.contains_key("fixtures") || object.contains_key("source")))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PatchLayer {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub order: i32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StageLayout {
    #[serde(default)]
    pub version: u64,
    #[serde(default, serialize_with = "serialize_stage_positions")]
    pub positions: HashMap<String, StagePosition2d>,
    #[serde(
        default,
        rename = "positions3d",
        serialize_with = "serialize_stage_positions"
    )]
    pub positions_3d: HashMap<String, StagePosition3d>,
    #[serde(default, rename = "camera3d", skip_serializing_if = "Option::is_none")]
    pub camera_3d: Option<StageCamera3d>,
    #[serde(
        default,
        rename = "positions2dConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub positions_2d_config: Option<StagePositions2dConfig>,
}

fn serialize_stage_positions<S, T>(
    positions: &HashMap<String, T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    positions
        .iter()
        .map(|(fixture_id, position)| (fixture_id.as_str(), position))
        .collect::<BTreeMap<_, _>>()
        .serialize(serializer)
}

impl StageLayout {
    /// Resolves the compatibility default without rewriting an untouched legacy object.
    ///
    /// A legacy layout containing operator-authored 2D positions is manual. An absent or empty
    /// 2D map is automatic and follows the default front projection.
    pub fn effective_positions_2d_config(&self) -> StagePositions2dConfig {
        self.positions_2d_config.unwrap_or(StagePositions2dConfig {
            provenance: if self.positions.is_empty() {
                StagePositions2dProvenance::Automatic
            } else {
                StagePositions2dProvenance::Manual
            },
            projection: StageProjection2d::default(),
        })
    }

    pub fn mark_positions_2d_manual(&mut self) {
        let projection = self.effective_positions_2d_config().projection;
        self.positions_2d_config = Some(StagePositions2dConfig {
            provenance: StagePositions2dProvenance::Manual,
            projection,
        });
    }

    /// Replaces the 2D positions with a deterministic orthographic projection of the 3D layout.
    ///
    /// Fixture ids are sorted before projection so hash-map insertion order cannot influence the
    /// serialized or in-memory result.
    pub fn regenerate_positions_2d(&mut self, projection: StageProjection2d) {
        self.positions = project_positions_2d(&self.positions_3d, projection);
        self.positions_2d_config = Some(StagePositions2dConfig {
            provenance: StagePositions2dProvenance::Automatic,
            projection,
        });
    }

    /// Refreshes automatic layouts after a 3D edit and preserves manual operator placement.
    pub fn refresh_automatic_positions_2d(&mut self) -> bool {
        let config = self.effective_positions_2d_config();
        if config.provenance == StagePositions2dProvenance::Manual {
            self.positions_2d_config = Some(config);
            return false;
        }
        self.regenerate_positions_2d(config.projection);
        true
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StagePosition2d {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagePosition3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub rotation_x: f64,
    #[serde(default)]
    pub rotation_y: f64,
    #[serde(default)]
    pub rotation_z: f64,
    /// Authored rectangular footprint for a scalable crowd-area Venue fixture. Other fixtures
    /// leave these absent, and legacy layouts remain valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crowd_width_metres: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crowd_depth_metres: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StagePositions2dProvenance {
    Automatic,
    Manual,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StagePositions2dConfig {
    pub provenance: StagePositions2dProvenance,
    pub projection: StageProjection2d,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageProjection2d {
    TopToBottom,
    BottomToTop,
    #[default]
    FrontToBack,
    BackToFront,
    LeftToRight,
    RightToLeft,
}

fn project_positions_2d(
    positions_3d: &HashMap<String, StagePosition3d>,
    projection: StageProjection2d,
) -> HashMap<String, StagePosition2d> {
    let mut projected = positions_3d
        .iter()
        .map(|(id, position)| {
            let (horizontal, vertical) = match projection {
                StageProjection2d::TopToBottom => (position.x, position.y),
                StageProjection2d::BottomToTop => (-position.x, position.y),
                StageProjection2d::FrontToBack => (position.x, position.z),
                StageProjection2d::BackToFront => (-position.x, position.z),
                StageProjection2d::LeftToRight => (position.y, position.z),
                StageProjection2d::RightToLeft => (-position.y, position.z),
            };
            (id.clone(), horizontal, vertical)
        })
        .collect::<Vec<_>>();
    projected.sort_by(|left, right| left.0.cmp(&right.0));
    if projected.is_empty() {
        return HashMap::new();
    }
    let (min_horizontal, max_horizontal, min_vertical, max_vertical) = projected.iter().fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |bounds, (_, horizontal, vertical)| {
            (
                bounds.0.min(*horizontal),
                bounds.1.max(*horizontal),
                bounds.2.min(*vertical),
                bounds.3.max(*vertical),
            )
        },
    );
    projected
        .into_iter()
        .map(|(id, horizontal, vertical)| {
            (
                id,
                StagePosition2d {
                    x: normalize_projection_coordinate(horizontal, min_horizontal, max_horizontal),
                    // 2D Stage uses screen coordinates, so greater world height appears nearer
                    // the top of the pane.
                    y: 100.0
                        - normalize_projection_coordinate(vertical, min_vertical, max_vertical),
                    rotation: 0.0,
                },
            )
        })
        .collect()
}

fn normalize_projection_coordinate(value: f64, minimum: f64, maximum: f64) -> f64 {
    const MARGIN_PERCENT: f64 = 5.0;
    const SPAN_PERCENT: f64 = 90.0;
    if (maximum - minimum).abs() <= f64::EPSILON {
        return 50.0;
    }
    MARGIN_PERCENT + (value - minimum) / (maximum - minimum) * SPAN_PERCENT
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct StageCamera3d {
    pub position: [f64; 3],
    pub target: [f64; 3],
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLayout {
    #[serde(default)]
    pub desks: Vec<PortableJson>,
    #[serde(default)]
    pub active_desk_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_settings: Option<PortableJson>,
}

impl ActiveShowObjectKind {
    pub fn from_storage_kind(kind: &str) -> Option<Self> {
        match kind {
            "attribute_configuration" => Some(Self::AttributeConfiguration),
            "cue_list" => Some(Self::CueList),
            "dynamic" => Some(Self::Dynamic),
            "group" => Some(Self::Group),
            "macro" => Some(Self::Macro),
            "patch_layer" => Some(Self::PatchLayer),
            "playback" => Some(Self::Playback),
            "playback_page" => Some(Self::PlaybackPage),
            "preset" => Some(Self::Preset),
            "psn" => Some(Self::Psn),
            "schedule" => Some(Self::Schedule),
            "stage_layout" => Some(Self::StageLayout),
            "timecode" => Some(Self::Timecode),
            "user_layout" => Some(Self::UserLayout),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AttributeConfiguration => "attribute_configuration",
            Self::CueList => "cue_list",
            Self::Dynamic => "dynamic",
            Self::Group => "group",
            Self::Macro => "macro",
            Self::PatchLayer => "patch_layer",
            Self::Playback => "playback",
            Self::PlaybackPage => "playback_page",
            Self::Preset => "preset",
            Self::Psn => "psn",
            Self::Schedule => "schedule",
            Self::StageLayout => "stage_layout",
            Self::Timecode => "timecode",
            Self::UserLayout => "user_layout",
        }
    }
}

/// One optimistic show-object edit within a whole-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectMutation {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
    pub mutation: ActiveShowObjectMutationKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveShowObjectMutationKind {
    Put { body: Box<ActiveShowObjectBody> },
    Delete,
}

/// One atomic batch of active-show object edits.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateActiveShowObjectsCommand {
    pub show_id: ShowId,
    pub mutations: Vec<ActiveShowObjectMutation>,
}

impl ApplicationCommand for MutateActiveShowObjectsCommand {
    type Value = MutateActiveShowObjectsResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectChange {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub object_revision: Revision,
    pub body: Option<ActiveShowObjectBody>,
    pub deleted: bool,
}

impl ActiveShowObjectChange {
    pub fn present(
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
        raw: serde_json::Value,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            kind,
            object_id,
            object_revision,
            body: Some(ActiveShowObjectBody::decode(kind, raw)?),
            deleted: false,
        })
    }

    pub fn deleted(
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
    ) -> Self {
        Self {
            kind,
            object_id,
            object_revision,
            body: None,
            deleted: true,
        }
    }
}

/// One committed semantic batch of active-show object changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectsChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub changes: Vec<ActiveShowObjectChange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MutateActiveShowObjectsResult {
    pub context: ActionContext,
    pub show_revision: PortableShowRevision,
    pub changes: Vec<ActiveShowObjectChange>,
    /// Compatibility-migration write-backs committed alongside the requested changes. Reported so
    /// adapters can publish their revision bumps instead of leaving them silent.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Route-kind compatibility-migration write-backs committed alongside the request.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub event_sequence: u64,
}

/// Restores the latest retained version of one object in the active portable show.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowObjectCommand {
    pub show_id: ShowId,
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
}

impl ApplicationCommand for UndoActiveShowObjectCommand {
    type Value = UndoActiveShowObjectResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowObjectResult {
    pub context: ActionContext,
    pub show_revision: PortableShowRevision,
    pub change: ActiveShowObjectChange,
    /// Compatibility-migration write-backs committed alongside the undone object. Reported so
    /// adapters can publish their revision bumps instead of leaving them silent.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Route-kind compatibility-migration write-backs committed alongside the undo.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndoActiveShowRecordingOperation {
    RestorePrevious,
    DeleteCreated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoActiveShowRecordingObject {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
    pub operation: UndoActiveShowRecordingOperation,
}

/// Reverses every portable object changed by one programmer recording in one transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowRecordingCommand {
    pub show_id: ShowId,
    pub objects: Vec<UndoActiveShowRecordingObject>,
}

impl ApplicationCommand for UndoActiveShowRecordingCommand {
    type Value = MutateActiveShowObjectsResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

/// One typed output-route edit performed against the active portable show.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteCommand {
    pub show_id: ShowId,
    pub route_id: String,
    /// Compatibility revision from the v1 object endpoint. The application service still commits
    /// the complete candidate against the document's whole-show revision.
    pub expected_object_revision: Revision,
    pub mutation: OutputRouteMutation,
}

/// One server-expanded, atomic sequence of paired output-route creations.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateOutputRouteRangeCommand {
    pub show_id: ShowId,
    /// Stable operation identity used to derive collision-resistant route object ids.
    pub range_id: uuid::Uuid,
    /// The first logical and destination universes live in this route candidate.
    pub first_route: LosslessBody<OutputRoute>,
    pub logical_universe_end: u16,
    pub destination_universe_end: u16,
}

impl ApplicationCommand for CreateOutputRouteRangeCommand {
    type Value = CreateOutputRouteRangeResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

impl ApplicationCommand for MutateOutputRouteCommand {
    type Value = MutateOutputRouteResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputRouteMutation {
    Put { body: LosslessBody<OutputRoute> },
    Delete,
}

/// Targeted active-show projection published after one committed route edit.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRouteChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub route_id: String,
    pub object_revision: Revision,
    pub route: Option<OutputRoute>,
    pub deleted: bool,
}

/// Mutation result plus the one old network route requiring targeted termination, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteResult {
    pub context: ActionContext,
    pub change: OutputRouteChange,
    /// Compatibility-migration write-backs of typed show objects committed alongside the route.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Compatibility-migration write-backs of other routes committed alongside the request.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub route_to_terminate: Option<OutputRoute>,
    pub event_sequence: u64,
}

/// Result of one all-or-nothing output-route range creation.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateOutputRouteRangeResult {
    pub context: ActionContext,
    pub changes: Vec<OutputRouteChange>,
    pub migration_changes: Vec<ActiveShowObjectChange>,
    pub migrated_routes: Vec<OutputRouteChange>,
    pub event_sequence: u64,
}

#[cfg(test)]
mod stage_layout_2d_tests {
    use super::*;

    fn position(x: f64, y: f64, z: f64) -> StagePosition3d {
        StagePosition3d {
            x,
            y,
            z,
            ..StagePosition3d::default()
        }
    }

    #[test]
    fn legacy_layouts_infer_manual_only_when_2d_positions_exist() {
        let manual: StageLayout = serde_json::from_value(serde_json::json!({
            "version": 2,
            "positions": {"fixture": {"x": 12.0, "y": 34.0, "rotation": 0.0}},
            "positions3d": {}
        }))
        .unwrap();
        assert_eq!(
            manual.effective_positions_2d_config().provenance,
            StagePositions2dProvenance::Manual
        );
        assert!(manual.positions_2d_config.is_none());

        let automatic: StageLayout = serde_json::from_value(serde_json::json!({
            "version": 2,
            "positions3d": {}
        }))
        .unwrap();
        assert_eq!(
            automatic.effective_positions_2d_config(),
            StagePositions2dConfig {
                provenance: StagePositions2dProvenance::Automatic,
                projection: StageProjection2d::FrontToBack,
            }
        );
        assert!(
            serde_json::to_value(&automatic)
                .unwrap()
                .get("positions2dConfig")
                .is_none(),
            "reading an untouched legacy object must not force a schema rewrite"
        );
    }

    #[test]
    fn regeneration_is_deterministic_across_3d_map_insertion_order() {
        let mut first = StageLayout::default();
        first
            .positions_3d
            .insert("b".into(), position(10.0, 5.0, 20.0));
        first
            .positions_3d
            .insert("a".into(), position(0.0, -5.0, 0.0));
        let mut second = StageLayout::default();
        second
            .positions_3d
            .insert("a".into(), position(0.0, -5.0, 0.0));
        second
            .positions_3d
            .insert("b".into(), position(10.0, 5.0, 20.0));

        first.regenerate_positions_2d(StageProjection2d::FrontToBack);
        second.regenerate_positions_2d(StageProjection2d::FrontToBack);

        assert_eq!(first.positions, second.positions);
        assert_eq!(first.positions["a"].x, 5.0);
        assert_eq!(first.positions["a"].y, 95.0);
        assert_eq!(first.positions["b"].x, 95.0);
        assert_eq!(first.positions["b"].y, 5.0);
    }

    #[test]
    fn automatic_refresh_preserves_manual_positions() {
        let mut layout = StageLayout {
            positions: HashMap::from([(
                "fixture".into(),
                StagePosition2d {
                    x: 17.0,
                    y: 29.0,
                    rotation: 45.0,
                },
            )]),
            positions_3d: HashMap::from([("fixture".into(), position(99.0, 88.0, 77.0))]),
            ..StageLayout::default()
        };

        assert!(!layout.refresh_automatic_positions_2d());
        assert_eq!(layout.positions["fixture"].x, 17.0);
        assert_eq!(
            layout.positions_2d_config.unwrap().provenance,
            StagePositions2dProvenance::Manual
        );
    }

    #[test]
    fn explicit_regeneration_replaces_manual_positions_and_restores_automatic_updates() {
        let mut layout = StageLayout {
            positions: HashMap::from([(
                "fixture".into(),
                StagePosition2d {
                    x: 17.0,
                    y: 29.0,
                    rotation: 45.0,
                },
            )]),
            positions_3d: HashMap::from([
                ("fixture".into(), position(0.0, 0.0, 0.0)),
                ("other".into(), position(10.0, 0.0, 10.0)),
            ]),
            ..StageLayout::default()
        };

        layout.regenerate_positions_2d(StageProjection2d::BackToFront);

        assert_eq!(
            layout.effective_positions_2d_config(),
            StagePositions2dConfig {
                provenance: StagePositions2dProvenance::Automatic,
                projection: StageProjection2d::BackToFront,
            }
        );
        assert_eq!(layout.positions["fixture"].x, 95.0);
        assert_eq!(layout.positions["fixture"].rotation, 0.0);
        layout.positions_3d.get_mut("fixture").unwrap().z = 20.0;
        assert!(layout.refresh_automatic_positions_2d());
        assert_eq!(layout.positions["fixture"].y, 5.0);
    }
}
