use super::*;

pub(super) fn default_autosave_interval_seconds() -> u64 {
    30
}

pub(super) fn default_speed_groups() -> [f64; 5] {
    [120.0, 90.0, 60.0, 30.0, 15.0]
}

pub(super) fn default_sound_to_light() -> [SoundToLightConfig; 5] {
    std::array::from_fn(|_| SoundToLightConfig::default())
}
pub(super) fn default_speed_group_sources() -> [SpeedGroupSource; 5] {
    std::array::from_fn(|_| SpeedGroupSource::Manual)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PoolColorMode {
    #[default]
    Type,
    Individual,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct PresetPoolColorPalette {
    pub(super) mixed: String,
    pub(super) intensity: String,
    pub(super) color: String,
    pub(super) position: String,
    pub(super) beam: String,
}

impl Default for PresetPoolColorPalette {
    fn default() -> Self {
        Self {
            mixed: "#89939e".into(),
            intensity: "#89939e".into(),
            color: "#89939e".into(),
            position: "#89939e".into(),
            beam: "#89939e".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct PoolColorPalette {
    pub(super) group: String,
    pub(super) macro_color: String,
    pub(super) dynamic: String,
    pub(super) cuelist: String,
    pub(super) sequence: String,
    pub(super) preset: PresetPoolColorPalette,
}

impl Default for PoolColorPalette {
    fn default() -> Self {
        Self {
            group: "#d8ad55".into(),
            macro_color: "#8f3541".into(),
            dynamic: "#3bbdce".into(),
            cuelist: "#93cc55".into(),
            sequence: "#93cc55".into(),
            preset: PresetPoolColorPalette::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct PoolItemPresentation {
    pub(super) title: Option<String>,
    pub(super) icon: Option<String>,
    pub(super) color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct PoolPresentationConfiguration {
    pub(super) palette: PoolColorPalette,
    /// Surface keys include the active show UUID plus a built-in type or stable pane ID.
    pub(super) modes: HashMap<String, PoolColorMode>,
    /// Item keys include the active show UUID, object type, and stable object ID.
    pub(super) items: HashMap<String, PoolItemPresentation>,
}

impl Default for PoolPresentationConfiguration {
    fn default() -> Self {
        Self {
            palette: PoolColorPalette::default(),
            modes: HashMap::new(),
            items: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum SpeedGroupSource {
    #[default]
    Manual,
    SpeedGroup {
        group: u8,
    },
    SoundToLight,
}
pub(super) fn deserialize_speed_groups<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<[f64; 5], D::Error> {
    let values = Vec::<f64>::deserialize(deserializer)?;
    if !(values.len() == 4 || values.len() == 5) {
        return Err(serde::de::Error::custom(
            "speed_groups_bpm requires four or five values",
        ));
    }
    let mut result = default_speed_groups();
    result[..values.len()].copy_from_slice(&values);
    Ok(result)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct DeskConfiguration {
    pub(super) frame_rate_hz: u16,
    pub(super) output_bind_ip: IpAddr,
    pub(super) osc_bind: Option<SocketAddr>,
    pub(super) art_timecode_bind: Option<SocketAddr>,
    pub(super) midi_inputs: Vec<String>,
    pub(super) rtp_midi_bind: Option<SocketAddr>,
    pub(super) timecode_sources: Vec<TimecodeSourceConfig>,
    pub(super) osc_timecode: Option<OscTimecodeConfig>,
    pub(super) backup_retention: usize,
    /// Seconds between automatic recovery checkpoints of the active show (api-rules §8).
    #[serde(default = "default_autosave_interval_seconds")]
    pub(super) autosave_interval_seconds: u64,
    #[serde(
        default = "default_speed_groups",
        deserialize_with = "deserialize_speed_groups"
    )]
    pub(super) speed_groups_bpm: [f64; 5],
    #[serde(default = "default_sound_to_light")]
    pub(super) speed_group_sound_to_light: [SoundToLightConfig; 5],
    #[serde(default = "default_speed_group_sources")]
    pub(super) speed_group_sources: [SpeedGroupSource; 5],
    pub(super) programmer_fade_millis: u64,
    /// Preserve the traditional command-line AT fade. When disabled, AT without an explicit TIME
    /// is immediate and records no per-value zero-second override.
    pub(super) command_line_at_uses_programmer_fade: bool,
    pub(super) sequence_master_fade_millis: u64,
    pub(super) preload_programmer_changes: bool,
    pub(super) preload_physical_playback_actions: bool,
    pub(super) preload_virtual_playback_actions: bool,
    /// Allow Show Patch's scoped Stage preview selection to identify fixtures on DMX.
    pub(super) patch_preview_highlight_dmx: bool,
    /// Installation-wide semantic Highlight contribution. A missing field means the installation
    /// predates this ownership boundary and its portable raw overrides require review.
    #[serde(default = "legacy_highlight_look")]
    pub(super) highlight_look: light_fixture::HighlightLook,
    /// An explicit installation-wide decision to ignore preserved legacy raw maps. This remains
    /// false for fresh defaults until a legacy show is actually encountered and reviewed.
    #[serde(default)]
    pub(super) highlight_legacy_overrides_acknowledged: bool,
    /// Desk-persistent opt-in for the global page/playback Matter bridge.
    pub(super) matter_enabled: bool,
    /// Pool colors are a desk presentation preference, never portable show content.
    pub(super) pool_presentation: PoolPresentationConfiguration,
    /// Workflow defaults belong to a concrete desk rather than to portable show data.
    pub(super) update_settings_by_desk: HashMap<Uuid, update::UpdateSettings>,
    pub(super) file_manager_system_picker_fallback: bool,
    pub(super) file_manager_roots: Vec<file_manager::ConfiguredRoot>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct OscTimecodeConfig {
    pub(super) address: String,
    pub(super) rate: FrameRate,
}
impl Default for DeskConfiguration {
    fn default() -> Self {
        Self {
            frame_rate_hz: 44,
            output_bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            osc_bind: Some(SocketAddr::from(([127, 0, 0, 1], 9000))),
            art_timecode_bind: None,
            midi_inputs: Vec::new(),
            rtp_midi_bind: None,
            timecode_sources: vec![
                TimecodeSourceConfig {
                    source_prefix: "artnet:".into(),
                    priority: 30,
                    fallback: false,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "midi:".into(),
                    priority: 20,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "rtp:".into(),
                    priority: 20,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
                TimecodeSourceConfig {
                    source_prefix: "osc:".into(),
                    priority: 10,
                    fallback: true,
                    loss_timeout_millis: 500,
                },
            ],
            osc_timecode: None,
            backup_retention: 20,
            autosave_interval_seconds: default_autosave_interval_seconds(),
            speed_groups_bpm: default_speed_groups(),
            speed_group_sound_to_light: default_sound_to_light(),
            speed_group_sources: default_speed_group_sources(),
            programmer_fade_millis: 3_000,
            command_line_at_uses_programmer_fade: true,
            sequence_master_fade_millis: 3_000,
            preload_programmer_changes: true,
            preload_physical_playback_actions: true,
            preload_virtual_playback_actions: false,
            patch_preview_highlight_dmx: false,
            highlight_look: light_fixture::HighlightLook::default(),
            highlight_legacy_overrides_acknowledged: false,
            matter_enabled: false,
            pool_presentation: PoolPresentationConfiguration::default(),
            update_settings_by_desk: HashMap::new(),
            file_manager_system_picker_fallback: false,
            file_manager_roots: Vec::new(),
        }
    }
}
impl DeskConfiguration {
    pub(super) fn migrate_speed_group_sources(&mut self) {
        for index in 0..self.speed_group_sources.len() {
            if self.speed_group_sources[index] == SpeedGroupSource::Manual
                && self.speed_group_sound_to_light[index].enabled
            {
                self.speed_group_sources[index] = SpeedGroupSource::SoundToLight;
            }
            self.speed_group_sound_to_light[index].enabled =
                self.speed_group_sources[index] == SpeedGroupSource::SoundToLight;
        }
    }

    pub(super) fn validate(&self) -> Result<(), ApiError> {
        if !(40..=44).contains(&self.frame_rate_hz) {
            return Err(ApiError::bad_request("frame_rate_hz must be 40-44"));
        }
        if self.backup_retention == 0 || self.backup_retention > 1_000 {
            return Err(ApiError::bad_request("backup_retention must be 1-1000"));
        }
        if !(5..=3_600).contains(&self.autosave_interval_seconds) {
            return Err(ApiError::bad_request(
                "autosave_interval_seconds must be 5-3600",
            ));
        }
        if self
            .speed_groups_bpm
            .iter()
            .any(|bpm| !bpm.is_finite() || !(0.1..=999.0).contains(bpm))
        {
            return Err(ApiError::bad_request(
                "speed_groups_bpm values must be finite and within 0.1-999",
            ));
        }
        for sound in &self.speed_group_sound_to_light {
            sound
                .validate()
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        validate_speed_group_source_graph(&self.speed_group_sources)?;
        if self.programmer_fade_millis > 60_000 || self.sequence_master_fade_millis > 60_000 {
            return Err(ApiError::bad_request(
                "fade times must be 0-60000 milliseconds",
            ));
        }
        self.highlight_look
            .validate()
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let mut root_ids = std::collections::HashSet::new();
        for root in &self.file_manager_roots {
            if root.id.trim().is_empty() || root.label.trim().is_empty() || !root.path.is_absolute()
            {
                return Err(ApiError::bad_request(
                    "File Manager roots require a stable ID, label, and absolute server path",
                ));
            }
            if !root_ids.insert(&root.id) {
                return Err(ApiError::bad_request(
                    "File Manager root IDs must be unique",
                ));
            }
        }
        self.pool_presentation.validate()?;
        Ok(())
    }
}

fn legacy_highlight_look() -> light_fixture::HighlightLook {
    light_fixture::HighlightLook::needs_review()
}

pub(super) fn wire_configuration_value(
    configuration: &DeskConfiguration,
) -> Result<serde_json::Value, ApiError> {
    let mut value = serde_json::to_value(configuration)
        .map_err(|error| ApiError::internal(error.to_string()))?;
    value["highlight_look"] =
        serde_json::to_value(wire_highlight_look(&configuration.highlight_look))
            .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok(value)
}

fn wire_highlight_look(
    value: &light_fixture::HighlightLook,
) -> light_wire::v2::desk_management::HighlightLookConfiguration {
    use light_wire::v2::desk_management as wire;
    wire::HighlightLookConfiguration {
        intensity: value.intensity,
        color: value.color.map(|color| match color {
            light_fixture::HighlightColor::White => wire::HighlightLookColor::White,
            light_fixture::HighlightColor::Red => wire::HighlightLookColor::Red,
            light_fixture::HighlightColor::Green => wire::HighlightLookColor::Green,
            light_fixture::HighlightColor::Blue => wire::HighlightLookColor::Blue,
            light_fixture::HighlightColor::Cyan => wire::HighlightLookColor::Cyan,
            light_fixture::HighlightColor::Magenta => wire::HighlightLookColor::Magenta,
            light_fixture::HighlightColor::Amber => wire::HighlightLookColor::Amber,
        }),
        iris: value.iris,
        zoom: value.zoom,
        focus: value.focus,
        frost: value.frost,
        compatibility: match value.compatibility {
            light_fixture::HighlightLookCompatibility::Semantic => {
                wire::HighlightLookCompatibility::Semantic
            }
            light_fixture::HighlightLookCompatibility::LegacyRaw => {
                wire::HighlightLookCompatibility::LegacyRaw
            }
            light_fixture::HighlightLookCompatibility::NeedsReview => {
                wire::HighlightLookCompatibility::NeedsReview
            }
        },
    }
}

#[cfg(test)]
mod highlight_look_tests {
    use super::*;

    #[test]
    fn fresh_configuration_is_semantic_but_missing_persisted_field_needs_review() {
        assert_eq!(
            DeskConfiguration::default().highlight_look.compatibility,
            light_fixture::HighlightLookCompatibility::Semantic
        );

        let mut legacy = serde_json::to_value(DeskConfiguration::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("highlight_look");
        let decoded: DeskConfiguration = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            decoded.highlight_look.compatibility,
            light_fixture::HighlightLookCompatibility::NeedsReview
        );
    }

    #[test]
    fn invalid_highlight_values_fail_configuration_validation() {
        let mut configuration = DeskConfiguration::default();
        configuration.highlight_look.frost = Some(-0.01);
        let error = configuration.validate().unwrap_err();
        assert!(error.message.contains("frost"), "{}", error.message);
    }

    #[test]
    fn configuration_transport_projects_named_abstract_color() {
        let mut configuration = DeskConfiguration::default();
        configuration.highlight_look.color = Some(light_fixture::HighlightColor::Blue);
        let wire = wire_configuration_value(&configuration).unwrap();
        assert_eq!(wire["highlight_look"]["color"], "blue");
        assert!(wire["highlight_look"].get("shutter").is_none());
    }
}

impl PoolPresentationConfiguration {
    fn validate(&self) -> Result<(), ApiError> {
        for color in [
            &self.palette.group,
            &self.palette.macro_color,
            &self.palette.dynamic,
            &self.palette.cuelist,
            &self.palette.sequence,
            &self.palette.preset.mixed,
            &self.palette.preset.intensity,
            &self.palette.preset.color,
            &self.palette.preset.position,
            &self.palette.preset.beam,
        ] {
            validate_pool_color(color)?;
        }
        if self.modes.len() > 1_024 || self.items.len() > 10_000 {
            return Err(ApiError::bad_request(
                "pool presentation contains too many surface or item entries",
            ));
        }
        for key in self.modes.keys().chain(self.items.keys()) {
            if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
                return Err(ApiError::bad_request(
                    "pool presentation keys must contain 1-256 printable characters",
                ));
            }
        }
        for item in self.items.values() {
            if let Some(color) = &item.color {
                validate_pool_color(color)?;
            }
            for value in [&item.title, &item.icon].into_iter().flatten() {
                if value.len() > 1_024 || value.chars().any(char::is_control) {
                    return Err(ApiError::bad_request(
                        "pool presentation labels must be printable and at most 1024 characters",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_pool_color(color: &str) -> Result<(), ApiError> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "pool presentation colors must use #RRGGBB",
        ))
    }
}

pub(super) fn validate_speed_group_source_graph(
    sources: &[SpeedGroupSource; 5],
) -> Result<(), ApiError> {
    for start in 0..sources.len() {
        let mut visited = [false; 5];
        let mut current = start;
        loop {
            if visited[current] {
                return Err(ApiError::bad_request(
                    "Speed Group sources must not contain direct or indirect cycles",
                ));
            }
            visited[current] = true;
            let SpeedGroupSource::SpeedGroup { group } = sources[current] else {
                break;
            };
            let Some(next) = usize::from(group).checked_sub(1) else {
                return Err(ApiError::bad_request("Speed Group source must be A-E"));
            };
            if next >= sources.len() {
                return Err(ApiError::bad_request("Speed Group source must be A-E"));
            }
            current = next;
        }
    }
    Ok(())
}
