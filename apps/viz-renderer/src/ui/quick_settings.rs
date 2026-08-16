//! The Quick Settings panel: what it offers, how it is driven, and how it is drawn.
//!
//! One row per setting an operator can reach without leaving the window. The rows are the model —
//! their labels, their values, and what a nudge does to each — and the drawing at the end of the
//! file is that same list laid out on screen.

use super::*;
use crate::settings::{Preferences, StagedConnection, parse_port};
use viz_render::Overlay;
use viz_scene::{ProviderKind, RenderQuality};

/// One Quick Settings row.
///
/// The fixed rows are the same every time the panel opens. The snapshot rows are not: there is one
/// per capture the operator has taken, so the list is built rather than declared.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Row {
    Source,
    Server,
    Port,
    User,
    Quality,
    Focus,
    Appearance,
    Background,
    Ambient,
    Exposure,
    Labels,
    ShowSelection,
    FloorGrid,
    FogAmount,
    LampFogCloudiness,
    LampFogTurbulence,
    LaserFogCloudiness,
    LaserFogTurbulence,
    LaserBrightness,
    CrowdAmount,
    Persistence,
    PersistenceFalloff,
    InputUniverse,
    InputProtocol,
    BlenderPath,
    /// One kept snapshot, by its position in the list, newest first.
    Snapshot(usize),
    Connect,
    Cancel,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QuickSettingsTab {
    Source,
    #[default]
    Rendering,
    Features,
    Snapshots,
}

impl QuickSettingsTab {
    const ALL: [Self; 4] = [
        Self::Source,
        Self::Rendering,
        Self::Features,
        Self::Snapshots,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Rendering => "Rendering",
            Self::Features => "Features",
            Self::Snapshots => "Snapshots",
        }
    }
}

/// The longest afterglow the setting offers.
///
/// A third of a second is already well past what an eye does and into obvious smearing; the
/// range exists so an operator can exaggerate the effect to see what it is doing, not because
/// anything above about a tenth is realistic.
pub(super) const MAX_PERSISTENCE: f32 = 0.3;

fn adjust_fog_character(
    value: &mut f32,
    delta: i32,
    label: &str,
    message: &mut String,
) -> QuickSettingsOutcome {
    *value = (*value + 0.05 * delta as f32).clamp(0.0, 1.0);
    *message = format!("{label} {:.0}%", *value * 100.0);
    QuickSettingsOutcome::AppliedLocally
}

impl Row {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Server => "Server",
            Self::Port => "Port",
            Self::User => "Desk user",
            Self::Quality => "Render quality",
            Self::Focus => "Focus",
            Self::Appearance => "Appearance",
            Self::Background => "Background color",
            Self::Ambient => "Environment brightness",
            Self::Exposure => "Exposure",
            Self::Labels => "Fixture / plan labels",
            Self::ShowSelection => "Show selection",
            Self::FloorGrid => "Floor grid",
            Self::FogAmount => "Fog amount",
            Self::LampFogCloudiness => "Lamp fog cloudiness",
            Self::LampFogTurbulence => "Lamp fog turbulence",
            Self::LaserFogCloudiness => "Laser fog cloudiness",
            Self::LaserFogTurbulence => "Laser fog turbulence",
            Self::LaserBrightness => "Laser brightness",
            Self::CrowdAmount => "Crowd amount",
            Self::Persistence => "Persistence of vision",
            Self::PersistenceFalloff => "Persistence falloff",
            Self::InputUniverse => "Input universe",
            Self::InputProtocol => "Arrives as",
            Self::BlenderPath => "Blender",
            Self::Snapshot(_) => "Snapshot",
            Self::Connect => "Connect / Reconnect",
            Self::Cancel => "Cancel without reconnecting",
        }
    }

    fn is_text(self) -> bool {
        matches!(
            self,
            Self::Server | Self::Port | Self::User | Self::BlenderPath
        )
    }

    /// The `0..=1` fraction to draw as a bar beside the value, for rows that have one.
    fn meter_fraction(self, preferences: &Preferences) -> Option<f32> {
        match self {
            Self::FogAmount => Some(preferences.atmosphere.amount),
            Self::LampFogCloudiness => Some(preferences.fog_variation.lamp_cloudiness),
            Self::LampFogTurbulence => Some(preferences.fog_variation.lamp_turbulence),
            Self::LaserFogCloudiness => Some(preferences.fog_variation.laser_cloudiness),
            Self::LaserFogTurbulence => Some(preferences.fog_variation.laser_turbulence),
            // Against the strongest the setting offers rather than against `1.0`, so the bar
            // still means something above the built-in strength.
            Self::LaserBrightness => {
                Some(preferences.laser_brightness / crate::settings::MAX_LASER_BRIGHTNESS)
            }
            Self::CrowdAmount => Some(preferences.crowd_amount),
            // Against the longest afterglow the setting offers, so the bar reads as a proportion
            // of the range rather than as an unanchored number of seconds.
            Self::Persistence => Some(preferences.persistence.decay_seconds / MAX_PERSISTENCE),
            Self::Ambient => Some(preferences.ambient),
            _ => None,
        }
    }
}

/// What the operator asked the application to do after a key press.
#[derive(Clone, Debug, PartialEq)]
pub enum QuickSettingsOutcome {
    None,
    Close,
    /// Apply the staged endpoint and stage a new connection.
    Connect {
        host: String,
        port: u16,
    },
    /// Export the kept snapshot at this position to a Blender file.
    ExportSnapshot(usize),
    /// The source changed; the application must switch providers atomically.
    SourceChanged(ProviderKind),
    /// Reframe the camera around the current rig.
    FocusView,
    /// Quality or fog changed and applies immediately without reconnecting.
    AppliedLocally,
    /// The staged edit is invalid; the message is shown in place.
    Invalid(String),
}

pub struct QuickSettings {
    pub open: bool,
    pub selected: usize,
    pub tab: QuickSettingsTab,
    pub editing: bool,
    pub staged: StagedConnection,
    pub staged_user: String,
    /// The Blender the operator named, empty to let the application find one. Unlike the
    /// connection fields this applies as it is typed: it changes nothing that is running.
    pub staged_blender: String,
    pub message: String,
    /// The universe the input rows are configuring.
    pub input_universe: u16,
    /// Whether the planning provider can be selected in this build.
    pub planner_available: bool,
    pub planner_unavailable_reason: String,
    /// The kept snapshots, as the application currently knows them. Refreshed every frame so a
    /// capture taken with the panel open appears in it.
    pub snapshots: Vec<crate::snapshots::SnapshotRow>,
    /// Where captures are being written, for the line under the list.
    pub snapshot_folder: String,
    /// The Blender the application found on its own, looked for when the panel opens rather than
    /// on every frame it is drawn.
    discovered_blender: Option<std::path::PathBuf>,
}

impl QuickSettings {
    pub fn new(preferences: &Preferences, planner_available: bool) -> Self {
        Self {
            open: false,
            selected: 0,
            tab: QuickSettingsTab::Rendering,
            editing: false,
            staged: StagedConnection::from_preferences(preferences),
            staged_user: preferences.user.clone(),
            staged_blender: preferences.blender.clone(),
            message: String::new(),
            input_universe: 1,
            planner_available,
            planner_unavailable_reason: "Not available in this build".into(),
            snapshots: Vec::new(),
            snapshot_folder: String::new(),
            discovered_blender: None,
        }
    }

    pub fn toggle(&mut self, preferences: &Preferences) {
        self.open = !self.open;
        if self.open {
            // Re-stage from the live connection so cancelling can never disturb it.
            self.staged = StagedConnection::from_preferences(preferences);
            self.staged_user = preferences.user.clone();
            self.staged_blender = preferences.blender.clone();
            // Looking for Blender walks the search path, so it happens when the panel opens and
            // not on every frame it is drawn on. An operator who installs Blender while the panel
            // is up closes and reopens it, which is what they would do anyway.
            self.discovered_blender = viz_snapshot::find_blender(None);
            self.message.clear();
            self.editing = false;
        }
    }

    /// Every row in the panel, in the order they are drawn.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = match self.tab {
            QuickSettingsTab::Source => vec![
                Row::Source,
                Row::Server,
                Row::Port,
                Row::User,
                Row::InputUniverse,
                Row::InputProtocol,
                Row::Connect,
            ],
            QuickSettingsTab::Rendering => vec![
                Row::Quality,
                Row::Focus,
                Row::Ambient,
                Row::Exposure,
                Row::FogAmount,
                Row::LampFogCloudiness,
                Row::LampFogTurbulence,
                Row::LaserFogCloudiness,
                Row::LaserFogTurbulence,
                Row::LaserBrightness,
                Row::Persistence,
                Row::Background,
                Row::Appearance,
                Row::PersistenceFalloff,
            ],
            QuickSettingsTab::Features => {
                vec![
                    Row::Labels,
                    Row::ShowSelection,
                    Row::FloorGrid,
                    Row::CrowdAmount,
                ]
            }
            QuickSettingsTab::Snapshots => {
                let mut rows = vec![Row::BlenderPath];
                rows.extend((0..self.snapshots.len()).map(Row::Snapshot));
                rows
            }
        };
        rows.push(Row::Cancel);
        rows
    }

    pub fn move_tab(&mut self, delta: isize) {
        if self.editing {
            return;
        }
        let current = QuickSettingsTab::ALL
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or_default() as isize;
        self.tab = QuickSettingsTab::ALL
            [(current + delta).rem_euclid(QuickSettingsTab::ALL.len() as isize) as usize];
        self.selected = 0;
        self.message.clear();
    }

    pub fn row(&self) -> Row {
        let rows = self.rows();
        rows[self.selected.min(rows.len() - 1)]
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.editing {
            return;
        }
        let count = self.rows().len() as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(count)) as usize;
    }

    /// Left/right adjust the focused row.
    pub fn adjust(&mut self, delta: i32, preferences: &mut Preferences) -> QuickSettingsOutcome {
        match self.row() {
            Row::Source => {
                let next = match self.staged.source {
                    ProviderKind::LightingDesk => ProviderKind::PlanningSoftware,
                    ProviderKind::PlanningSoftware => ProviderKind::LightingDesk,
                };
                if next == ProviderKind::PlanningSoftware && !self.planner_available {
                    self.message = format!(
                        "Planning software \u{2014} {}",
                        self.planner_unavailable_reason
                    );
                    return QuickSettingsOutcome::Invalid(self.message.clone());
                }
                self.staged.source = next;
                self.message = "Press Enter on Connect to switch source".into();
                QuickSettingsOutcome::None
            }
            Row::Quality => {
                preferences.quality_override = cycle_quality(preferences.quality_override, delta);
                self.message = format!("Rendering quality: {}", preferences.quality_label());
                QuickSettingsOutcome::AppliedLocally
            }
            Row::Exposure => {
                preferences.exposure =
                    (preferences.exposure + 0.05 * delta as f32).clamp(0.05, 4.0);
                self.message = format!("Exposure {:.2}×", preferences.exposure);
                QuickSettingsOutcome::AppliedLocally
            }
            Row::Appearance => {
                preferences.theme = preferences.theme.toggled();
                self.message = format!("Appearance: {}", preferences.theme.label());
                QuickSettingsOutcome::AppliedLocally
            }
            Row::Background => {
                const COLORS: [Option<[f32; 3]>; 5] = [
                    None,
                    Some([0.0, 0.0, 0.0]),
                    Some([0.015, 0.025, 0.05]),
                    Some([0.04, 0.055, 0.09]),
                    Some([0.08, 0.08, 0.08]),
                ];
                let current = COLORS
                    .iter()
                    .position(|color| *color == preferences.background)
                    .unwrap_or_default() as i32;
                preferences.background =
                    COLORS[(current + delta).rem_euclid(COLORS.len() as i32) as usize];
                self.message = format!("Background {}", background_label(preferences.background));
                QuickSettingsOutcome::AppliedLocally
            }
            Row::Ambient => {
                preferences.ambient = (preferences.ambient + 0.02 * delta as f32).clamp(0.0, 1.0);
                self.message = format!("Ambient light {:.0}%", preferences.ambient * 100.0);
                QuickSettingsOutcome::AppliedLocally
            }
            Row::InputUniverse => {
                self.input_universe = self
                    .input_universe
                    .saturating_add_signed(delta as i16)
                    .max(1);
                self.message = format!("Universe {}", self.input_universe);
                QuickSettingsOutcome::None
            }
            Row::InputProtocol => {
                let universe = self.input_universe;
                let next = cycle_input(preferences.input_for(universe), delta);
                preferences.set_input(universe, next);
                self.message = match next {
                    None => format!("Universe {universe} follows the show's routes"),
                    Some(protocol) => format!(
                        "Universe {universe} arrives as {} on port {}",
                        protocol_label(protocol),
                        protocol.default_port()
                    ),
                };
                // Where a universe arrives is part of how the receivers are built, so it takes
                // effect on the next connection rather than silently half-applying.
                QuickSettingsOutcome::None
            }
            Row::Labels => {
                preferences.show_labels = !preferences.show_labels;
                self.message = if preferences.show_labels {
                    "Fixture numbers and addresses shown in the plan views".into()
                } else {
                    "Plan labels hidden".into()
                };
                QuickSettingsOutcome::AppliedLocally
            }
            Row::ShowSelection => {
                preferences.show_selection = !preferences.show_selection;
                self.message = if preferences.show_selection {
                    "Selected fixtures are highlighted".into()
                } else {
                    "Selection highlight hidden".into()
                };
                QuickSettingsOutcome::AppliedLocally
            }
            Row::FloorGrid => {
                preferences.floor_grid = match (preferences.floor_grid, delta.signum()) {
                    (None, 1) | (Some(false), -1) => Some(true),
                    (Some(true), 1) | (None, -1) => Some(false),
                    _ => None,
                };
                self.message =
                    format!("Floor grid {}", optional_bool_label(preferences.floor_grid));
                QuickSettingsOutcome::AppliedLocally
            }
            Row::FogAmount => {
                let step = 0.05 * delta as f32;
                preferences.atmosphere.amount =
                    (preferences.atmosphere.amount + step).clamp(0.0, 1.0);
                self.message = format!("Fog amount {:.0}%", preferences.atmosphere.amount * 100.0);
                QuickSettingsOutcome::AppliedLocally
            }
            Row::LampFogCloudiness => adjust_fog_character(
                &mut preferences.fog_variation.lamp_cloudiness,
                delta,
                "Lamp fog cloudiness",
                &mut self.message,
            ),
            Row::LampFogTurbulence => adjust_fog_character(
                &mut preferences.fog_variation.lamp_turbulence,
                delta,
                "Lamp fog turbulence",
                &mut self.message,
            ),
            Row::LaserFogCloudiness => adjust_fog_character(
                &mut preferences.fog_variation.laser_cloudiness,
                delta,
                "Laser fog cloudiness",
                &mut self.message,
            ),
            Row::LaserFogTurbulence => adjust_fog_character(
                &mut preferences.fog_variation.laser_turbulence,
                delta,
                "Laser fog turbulence",
                &mut self.message,
            ),
            Row::LaserBrightness => {
                let step = 0.1 * delta as f32;
                preferences.laser_brightness = (preferences.laser_brightness + step)
                    .clamp(0.0, crate::settings::MAX_LASER_BRIGHTNESS);
                self.message = if preferences.laser_brightness <= 0.0 {
                    "Lasers off in this window: the rig is drawn without them".into()
                } else {
                    format!(
                        "Lasers at {:.0}% — beams and the figures they draw together",
                        preferences.laser_brightness * 100.0
                    )
                };
                QuickSettingsOutcome::AppliedLocally
            }
            Row::CrowdAmount => {
                let step = 0.05 * delta as f32;
                preferences.crowd_amount = (preferences.crowd_amount + step).clamp(0.0, 1.0);
                self.message = format!(
                    "Crowd amount {:.0}% — portable footprints and seeds unchanged",
                    preferences.crowd_amount * 100.0
                );
                QuickSettingsOutcome::AppliedLocally
            }
            Row::Persistence => {
                let step = 0.02 * delta as f32;
                preferences.persistence.decay_seconds =
                    (preferences.persistence.decay_seconds + step).clamp(0.0, MAX_PERSISTENCE);
                self.message = if preferences.persistence.decay_seconds <= 0.0 {
                    "Persistence off: every frame shows exactly what the desk is sending".into()
                } else {
                    format!(
                        "Persistence {:.0} ms from full to black",
                        preferences.persistence.decay_seconds * 1000.0
                    )
                };
                QuickSettingsOutcome::AppliedLocally
            }
            Row::PersistenceFalloff => {
                let step = 0.25 * delta as f32;
                preferences.persistence.falloff =
                    (preferences.persistence.falloff + step).clamp(1.0, 8.0);
                self.message = format!(
                    "Persistence falloff {:.2} — {}",
                    preferences.persistence.falloff,
                    if preferences.persistence.falloff <= 1.05 {
                        "a straight fade"
                    } else {
                        "the tail drops away faster than the flash"
                    }
                );
                QuickSettingsOutcome::AppliedLocally
            }
            _ => QuickSettingsOutcome::None,
        }
    }

    /// Enter activates the focused row.
    pub fn activate(&mut self, preferences: &mut Preferences) -> QuickSettingsOutcome {
        let row = self.row();
        if row.is_text() {
            self.editing = !self.editing;
            // Where Blender is changes nothing that is running, so it is kept the moment editing
            // finishes rather than waiting behind Connect with the endpoint fields.
            if !self.editing && row == Row::BlenderPath {
                preferences.blender = self.staged_blender.trim().to_owned();
            }
            self.message = if self.editing {
                "Type to edit, Enter to accept".into()
            } else {
                String::new()
            };
            return QuickSettingsOutcome::None;
        }
        match row {
            Row::Focus => QuickSettingsOutcome::FocusView,
            Row::Snapshot(index) => {
                let Some(snapshot) = self.snapshots.get(index) else {
                    return QuickSettingsOutcome::None;
                };
                if snapshot.export == crate::snapshots::ExportState::Running {
                    self.message = "That snapshot is already being exported".into();
                    return QuickSettingsOutcome::None;
                }
                self.message = format!(
                    "Exporting the {} snapshot \u{2014} Blender writes it into {}",
                    snapshot.entry.label(),
                    snapshot.entry.directory.display()
                );
                QuickSettingsOutcome::ExportSnapshot(index)
            }
            Row::Connect => match self.staged.validate() {
                Ok((host, port)) => {
                    preferences.user = self.staged_user.trim().to_owned();
                    if self.staged.source != preferences.source {
                        preferences.source = self.staged.source;
                        preferences.host = host.clone();
                        preferences.port = port;
                        return QuickSettingsOutcome::SourceChanged(self.staged.source);
                    }
                    preferences.host = host.clone();
                    preferences.port = port;
                    self.message = format!("Connecting to {host}:{port}");
                    QuickSettingsOutcome::Connect { host, port }
                }
                Err(error) => {
                    self.message = error.clone();
                    QuickSettingsOutcome::Invalid(error)
                }
            },
            Row::Cancel => {
                self.staged = StagedConnection::from_preferences(preferences);
                self.staged_user = preferences.user.clone();
                self.open = false;
                self.message.clear();
                QuickSettingsOutcome::Close
            }
            Row::Quality
            | Row::Exposure
            | Row::Background
            | Row::LaserBrightness
            | Row::Persistence
            | Row::PersistenceFalloff
            | Row::FogAmount
            | Row::LampFogCloudiness
            | Row::LampFogTurbulence
            | Row::LaserFogCloudiness
            | Row::LaserFogTurbulence
            | Row::Source
            | Row::Appearance
            | Row::Ambient
            | Row::Labels => self.adjust(1, preferences),
            Row::ShowSelection | Row::FloorGrid => self.adjust(1, preferences),
            _ => QuickSettingsOutcome::None,
        }
    }

    pub fn type_character(&mut self, character: char) {
        if !self.editing {
            return;
        }
        match self.row() {
            Row::Server => self.staged.host.push(character),
            Row::Port if character.is_ascii_digit() => {
                self.staged.port_text.push(character);
            }
            Row::User => self.staged_user.push(character),
            Row::BlenderPath => self.staged_blender.push(character),
            _ => {}
        }
        self.validate_in_place();
    }

    pub fn backspace(&mut self) {
        if !self.editing {
            return;
        }
        match self.row() {
            Row::Server => {
                self.staged.host.pop();
            }
            Row::Port => {
                self.staged.port_text.pop();
            }
            Row::User => {
                self.staged_user.pop();
            }
            Row::BlenderPath => {
                self.staged_blender.pop();
            }
            _ => {}
        }
        self.validate_in_place();
    }

    fn validate_in_place(&mut self) {
        if self.row() == Row::Port
            && !self.staged.port_text.is_empty()
            && let Err(error) = parse_port(&self.staged.port_text)
        {
            self.message = error;
            return;
        }
        self.message.clear();
    }

    pub(super) fn value(&self, row: Row, preferences: &Preferences) -> String {
        match row {
            Row::Source => {
                if self.staged.source == ProviderKind::PlanningSoftware && !self.planner_available {
                    format!(
                        "Planning software \u{2014} {}",
                        self.planner_unavailable_reason
                    )
                } else {
                    self.staged.source.label().to_owned()
                }
            }
            Row::Server => self.staged.host.clone(),
            Row::Port => self.staged.port_text.clone(),
            Row::User => self.staged_user.clone(),
            Row::Quality => preferences.quality_label(),
            Row::Focus => "Frame rig".into(),
            Row::Appearance => preferences.theme.label().to_owned(),
            Row::Background => background_label(preferences.background),
            Row::Ambient => format!("{:.0}%", preferences.ambient * 100.0),
            Row::Exposure => format!("{:.2}×", preferences.exposure),
            Row::Labels => bool_label(preferences.show_labels),
            Row::ShowSelection => bool_label(preferences.show_selection),
            Row::FloorGrid => optional_bool_label(preferences.floor_grid),
            Row::FogAmount => format!("{:.0}%", preferences.atmosphere.amount * 100.0),
            Row::LampFogCloudiness => {
                format!("{:.0}%", preferences.fog_variation.lamp_cloudiness * 100.0)
            }
            Row::LampFogTurbulence => {
                format!("{:.0}%", preferences.fog_variation.lamp_turbulence * 100.0)
            }
            Row::LaserFogCloudiness => {
                format!("{:.0}%", preferences.fog_variation.laser_cloudiness * 100.0)
            }
            Row::LaserFogTurbulence => {
                format!("{:.0}%", preferences.fog_variation.laser_turbulence * 100.0)
            }
            Row::LaserBrightness => format!("{:.0}%", preferences.laser_brightness * 100.0),
            Row::CrowdAmount => format!("{:.0}%", preferences.crowd_amount * 100.0),
            Row::Persistence => {
                if preferences.persistence.decay_seconds <= 0.0 {
                    "off".into()
                } else {
                    format!("{:.0} ms", preferences.persistence.decay_seconds * 1000.0)
                }
            }
            Row::PersistenceFalloff => format!("{:.2}", preferences.persistence.falloff),
            Row::InputUniverse => format!("{}", self.input_universe),
            Row::InputProtocol => match preferences.input_for(self.input_universe) {
                None => "Follow show routes".into(),
                Some(protocol) => format!(
                    "{} port {}",
                    protocol_label(protocol),
                    protocol.default_port()
                ),
            },
            Row::BlenderPath => self.blender_value(),
            Row::Snapshot(index) => match self.snapshots.get(index) {
                Some(snapshot) => snapshot.status(),
                None => String::new(),
            },
            Row::Connect => "Enter".into(),
            Row::Cancel => "Esc".into(),
        }
    }

    /// The label a snapshot row carries: the time it was taken, which is what an operator
    /// remembers about the look they want back.
    pub fn row_label(&self, row: Row) -> String {
        match row {
            Row::Snapshot(index) => match self.snapshots.get(index) {
                Some(snapshot) => format!("Snapshot {}", snapshot.entry.label()),
                None => "Snapshot".to_owned(),
            },
            other => other.label().to_owned(),
        }
    }

    /// What the Blender row says: the path being used, or that nothing was found.
    ///
    /// An operator has to be able to tell "I have not set this and it found one anyway" from
    /// "there is nothing here and exporting will fail", so the row answers both.
    fn blender_value(&self) -> String {
        let typed = self.staged_blender.trim();
        if !typed.is_empty() {
            return typed.to_owned();
        }
        match &self.discovered_blender {
            Some(found) => format!("Found: {}", found.display()),
            None => "Not found \u{2014} type a path to blender".to_owned(),
        }
    }
}

fn background_label(color: Option<[f32; 3]>) -> String {
    let Some(color) = color else {
        return "Follow source".into();
    };
    format!(
        "#{:02X}{:02X}{:02X}",
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

fn optional_bool_label(value: Option<bool>) -> String {
    match value {
        None => "Follow source".into(),
        Some(value) => bool_label(value),
    }
}

/// `None` is the show's own routes; the two protocols follow.
pub(super) fn cycle_input(
    current: Option<viz_dmx::Protocol>,
    delta: i32,
) -> Option<viz_dmx::Protocol> {
    let order = [
        None,
        Some(viz_dmx::Protocol::ArtNet),
        Some(viz_dmx::Protocol::Sacn),
    ];
    let index = order
        .iter()
        .position(|candidate| *candidate == current)
        .unwrap_or(0) as i32;
    let next = (index + delta).rem_euclid(order.len() as i32) as usize;
    order[next]
}

pub(super) fn protocol_label(protocol: viz_dmx::Protocol) -> &'static str {
    match protocol {
        viz_dmx::Protocol::ArtNet => "Art-Net",
        viz_dmx::Protocol::Sacn => "sACN",
    }
}

pub(super) fn bool_label(value: bool) -> String {
    if value {
        "Enabled".into()
    } else {
        "Disabled".into()
    }
}

pub(super) fn cycle_quality(current: Option<RenderQuality>, delta: i32) -> Option<RenderQuality> {
    // `None` is "Follow source" and sits before the explicit tiers.
    let order: Vec<Option<RenderQuality>> = std::iter::once(None)
        .chain(RenderQuality::ALL.into_iter().map(Some))
        .collect();
    let index = order
        .iter()
        .position(|entry| *entry == current)
        .unwrap_or(0) as i32;
    let next = (index + delta).rem_euclid(order.len() as i32) as usize;
    order[next]
}

/// Build the Quick Settings panel.
pub fn build_quick_settings(
    overlay: &mut Overlay,
    settings: &QuickSettings,
    preferences: &Preferences,
    model: &StatusModel<'_>,
    width: f32,
    height: f32,
) {
    if !settings.open {
        return;
    }
    let palette = Palette::of(model.theme);
    let scale = ui_scale(width);
    let line = Overlay::line_height(scale) * 1.4;
    let rows = settings.rows();
    let panel_width = (620.0 * scale).min(width - 40.0);

    // The snapshot list grows with the operator's own work, so the panel cannot be sized from the
    // row count alone: past a certain number of captures it would be taller than the window, and a
    // keyboard-driven list that runs off the bottom of the screen cannot be reached at all. The
    // rows that fit are shown, moving with the selection, and the rest are counted underneath.
    let chrome_lines = if settings.snapshot_folder.is_empty() {
        9.0
    } else {
        10.0
    } + if model.degraded { 1.0 } else { 0.0 };
    // A line is held back for the list's own heading, and the margin keeps the panel and its
    // border clear of the window edge at every scale.
    let room = ((height - 72.0) / line - chrome_lines - 1.0)
        .floor()
        .max(4.0) as usize;
    let (first, visible) = visible_window(rows.len(), settings.selected, room);
    let shown = &rows[first..first + visible];
    let hidden = rows.len() - visible;
    // The list keeps its heading when it is scrolled into, so a column of times is never left
    // sitting under the connection fields with nothing to say what it is.
    let heading = usize::from(shown.iter().any(|row| matches!(row, Row::Snapshot(_))));

    let panel_height = line * ((visible + heading) as f32 + chrome_lines);
    let x = (width - panel_width) * 0.5;
    let y = ((height - panel_height) * 0.35).max(20.0);
    overlay.rect(
        x - 2.0,
        y - 2.0,
        panel_width + 4.0,
        panel_height + 4.0,
        palette.accent,
    );
    overlay.rect(x, y, panel_width, panel_height, palette.panel);

    let padding = 16.0 * scale;
    let mut cursor = y + padding;
    overlay.text(
        x + padding,
        cursor,
        scale * 1.4,
        palette.text,
        "Quick Settings",
    );
    cursor += line * 1.3;

    let mut tab_x = x + padding;
    for tab in QuickSettingsTab::ALL {
        let label = tab.label();
        let colour = if tab == settings.tab {
            palette.accent
        } else {
            palette.dim
        };
        tab_x += overlay.text(tab_x, cursor, scale, colour, label) + 18.0 * scale;
    }
    overlay.text(
        x + panel_width - padding - Overlay::measure("Tab changes page", scale),
        cursor,
        scale,
        palette.dim,
        "Tab changes page",
    );
    cursor += line;

    draw_rows(
        overlay,
        settings,
        preferences,
        &palette,
        shown,
        first,
        x,
        panel_width,
        &mut cursor,
        line,
        scale,
        padding,
    );

    cursor += line * 0.4;

    // A row that is out of sight must be accounted for, or a panel that has quietly stopped
    // showing half the captures looks like a panel that has lost them.
    if hidden > 0 {
        overlay.text(
            x + padding,
            cursor - line * 0.4,
            scale,
            palette.dim,
            &format!("{hidden} more \u{2014} keep scrolling"),
        );
    }
    if !settings.message.is_empty() {
        overlay.clipped_text(
            x + padding,
            cursor,
            scale,
            palette.warn,
            &settings.message,
            x + panel_width - padding,
        );
    }
    cursor += line;
    if !settings.snapshot_folder.is_empty() {
        // A capture is a file the operator will go and look for, so the panel says where it is
        // rather than leaving them to guess at an application data folder.
        overlay.clipped_text(
            x + padding,
            cursor,
            scale,
            palette.dim,
            &format!("Snapshots are kept in {}", settings.snapshot_folder),
            x + panel_width - padding,
        );
        cursor += line;
    }
    let diagnostic_rows = diagnostic_rows(model);
    for row in &diagnostic_rows[..2] {
        overlay.clipped_text(
            x + padding,
            cursor,
            scale,
            palette.dim,
            row,
            x + panel_width - padding,
        );
        cursor += line;
    }
    if let Some(reason) = model.quality_reduction_reason() {
        overlay.clipped_text(
            x + padding,
            cursor,
            scale,
            palette.warn,
            &reason,
            x + panel_width - padding,
        );
        cursor += line;
    }
    // Which GPU is drawing this, and how fast values are arriving to draw with. The connection
    // surface has to name the renderer backend, and an operator diagnosing a slow picture needs it
    // where the rest of the connection is rather than in a benchmark report.
    overlay.clipped_text(
        x + padding,
        cursor,
        scale,
        palette.dim,
        &diagnostic_rows[2],
        x + panel_width - padding,
    );
    cursor += line;
    overlay.text(
        x + padding,
        cursor,
        scale,
        palette.dim,
        "Arrows move and adjust, Enter activates, Esc cancels",
    );
}

pub(super) fn diagnostic_rows(model: &StatusModel<'_>) -> [String; 3] {
    let show = if model.diagnostics.show_identity.is_empty() {
        "-"
    } else {
        &model.diagnostics.show_identity
    };
    [
        format!(
            "{}  |  revision {}",
            model.connection.summary(),
            model.diagnostics.scene_revision
        ),
        format!("show {show}"),
        model.renderer_summary(),
    ]
}

/// Which stretch of rows to draw when they do not all fit.
///
/// Returns where the drawn stretch starts and how long it is. The focused row is always inside it,
/// because a keyboard-driven panel whose selection has scrolled out of sight is a panel the
/// operator is working blind in.
/// Draw the rows that fit, in the order the panel offers them.
///
/// One row is one setting: its name on the left, its value on the right, and a bar beside the
/// value where the setting has a range rather than a list of choices.
#[allow(clippy::too_many_arguments)]
fn draw_rows(
    overlay: &mut Overlay,
    settings: &QuickSettings,
    preferences: &Preferences,
    palette: &Palette,
    shown: &[Row],
    first: usize,
    x: f32,
    panel_width: f32,
    cursor: &mut f32,
    line: f32,
    scale: f32,
    padding: f32,
) {
    let mut headed = false;
    let cursor = &mut *cursor;
    for (offset, row) in shown.iter().copied().enumerate() {
        let index = first + offset;
        let focused = index == settings.selected;
        // The snapshot list is the one place the panel holds a growing list of the operator's own
        // work, so it gets a heading rather than starting mid-column with no explanation.
        if !headed && matches!(row, Row::Snapshot(_)) {
            headed = true;
            overlay.text(x + padding, *cursor, scale, palette.dim, "Snapshots");
            *cursor += line;
        }
        if focused {
            overlay.rect(
                x + 6.0,
                *cursor - 3.0 * scale,
                panel_width - 12.0,
                line,
                palette.focus,
            );
        }
        overlay.text(
            x + padding,
            *cursor,
            scale,
            if focused { palette.accent } else { palette.dim },
            &settings.row_label(row),
        );
        let value = settings.value(row, preferences);
        let editing_marker = if focused && settings.editing { "_" } else { "" };
        let value_ink = if matches!(row, Row::Snapshot(index) if settings
            .snapshots
            .get(index)
            .is_some_and(crate::snapshots::SnapshotRow::is_failure))
        {
            palette.bad
        } else if focused {
            palette.text
        } else {
            palette.dim
        };
        overlay.clipped_text(
            x + panel_width * 0.42,
            *cursor,
            scale,
            value_ink,
            &format!("{value}{editing_marker}"),
            x + panel_width - padding,
        );
        if let Some(fraction) = row.meter_fraction(preferences) {
            overlay.meter(
                x + panel_width * 0.42 + Overlay::measure("Follow source__", scale),
                *cursor + 2.0 * scale,
                panel_width * 0.2,
                5.0 * scale,
                fraction,
                palette.focus,
                palette.accent,
            );
        }
        *cursor += line;
    }
}

pub(super) fn visible_window(rows: usize, selected: usize, room: usize) -> (usize, usize) {
    let visible = rows.min(room.max(1));
    if visible == 0 {
        return (0, 0);
    }
    // Keep the selection roughly in the middle, and stop at either end of the list.
    let first = selected
        .min(rows.saturating_sub(1))
        .saturating_sub(visible / 2)
        .min(rows - visible);
    (first, visible)
}
