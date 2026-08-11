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

/// Which way a connected visualizer is pointing, and how hard it is working.
///
/// Desk-level presentation state, like the pool colours beside it: it describes a renderer this
/// installation drives, not anything the show file should carry to another building. A target
/// nobody has configured is simply the default view, so nothing needs migrating.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct VisualizerView {
    pub(super) mode: VisualizerViewMode,
    pub(super) quality: VisualizerRenderQuality,
    pub(super) camera: Option<VisualizerCamera>,
    pub(super) exposure: f32,
    pub(super) ambient: f32,
    pub(super) revision: u64,
}

impl Default for VisualizerView {
    fn default() -> Self {
        Self {
            mode: VisualizerViewMode::Full3d,
            quality: VisualizerRenderQuality::High,
            camera: None,
            exposure: 1.0,
            ambient: 0.06,
            revision: 0,
        }
    }
}

impl VisualizerView {
    pub(super) fn validate(&self) -> Result<(), ApiError> {
        if !self.exposure.is_finite() || !(0.05..=4.0).contains(&self.exposure) {
            return Err(ApiError::bad_request("exposure must be within 0.05-4.0"));
        }
        if !self.ambient.is_finite() || !(0.0..=1.0).contains(&self.ambient) {
            return Err(ApiError::bad_request("ambient must be within 0-1"));
        }
        if let Some(camera) = &self.camera {
            camera.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum VisualizerViewMode {
    TopDown,
    LeftToRight,
    RightToLeft,
    FrontToBack,
    BackToFront,
    #[serde(rename = "lines_3d")]
    Lines3d,
    #[serde(rename = "simple_3d")]
    Simple3d,
    #[default]
    #[serde(rename = "full_3d")]
    Full3d,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum VisualizerRenderQuality {
    Draft,
    Standard,
    #[default]
    High,
    Ultra,
}

/// Position, aim and up in stage metres, with no Euler order to disagree about.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct VisualizerCamera {
    pub(super) position: [f32; 3],
    pub(super) target: [f32; 3],
    pub(super) up: [f32; 3],
    pub(super) fov_degrees: f32,
    pub(super) orthographic_size: f32,
}

impl VisualizerCamera {
    fn validate(&self) -> Result<(), ApiError> {
        let finite = |values: &[f32; 3]| values.iter().all(|value| value.is_finite());
        if !finite(&self.position) || !finite(&self.target) || !finite(&self.up) {
            return Err(ApiError::bad_request(
                "camera position, target and up must be finite",
            ));
        }
        if self.up == [0.0; 3] {
            return Err(ApiError::bad_request("camera up must not be zero"));
        }
        if !self.fov_degrees.is_finite() || !(1.0..=170.0).contains(&self.fov_degrees) {
            return Err(ApiError::bad_request(
                "camera fov_degrees must be within 1-170",
            ));
        }
        if !self.orthographic_size.is_finite()
            || !(0.01..=10_000.0).contains(&self.orthographic_size)
        {
            return Err(ApiError::bad_request(
                "camera orthographic_size must be within 0.01-10000",
            ));
        }
        Ok(())
    }
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
    pub(super) timecode_source: TimecodeSourceSelection,
    /// `None` follows the configured DMX frame rate. `Some` is the operator's explicit override.
    pub(super) timecode_frame_rate: Option<DeskTimecodeFrameRate>,
    pub(super) timecode_external_loss_policy: ExternalTimecodeLossPolicy,
    pub(super) timecode_external_loss_timeout_millis: u64,
    pub(super) osc_timecode: Option<OscTimecodeConfig>,
    /// `None` follows the operating system default. A name is exact and never silently falls back.
    pub(super) timecode_audio_output_device: Option<String>,
    /// Per-destination operator calibration. The `$system_default` key owns default-device trim.
    pub(super) timecode_audio_latency_trim_micros_by_output: BTreeMap<String, i64>,
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
    /// Installation default copied onto each newly created Cuelist. Existing Cuelists retain
    /// their persisted behavior when this default changes.
    pub(super) cuelist_auto_off_at_zero_default: bool,
    /// Installation default copied onto each newly created Cuelist. Flash auto-off remains
    /// independent from fader-zero auto-off.
    pub(super) cuelist_auto_off_flash_release_default: bool,
    /// Start only a transactionally new Playback/Cuelist/first-Cue topology after recording.
    pub(super) start_after_first_recording: bool,
    pub(super) preload_programmer_changes: bool,
    pub(super) preload_physical_playback_actions: bool,
    pub(super) preload_virtual_playback_actions: bool,
    /// Allow Show Patch's scoped Stage preview selection to identify fixtures on DMX.
    pub(super) patch_preview_highlight_dmx: bool,
    /// Installation-wide semantic Highlight contribution. A missing field means the installation
    /// predates this ownership boundary and simply adopts the semantic default look; a show that
    /// really carries raw overrides is still caught by the compatibility scanner when it opens.
    #[serde(default)]
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
    /// What each connected visualizer is being told to look at, by target name. An installation
    /// that has never driven one carries nothing here.
    #[serde(default)]
    pub(super) visualizer_views: BTreeMap<String, VisualizerView>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct OscTimecodeConfig {
    pub(super) address: String,
    pub(super) rate: FrameRate,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub(super) struct DeskTimecodeFrameRate {
    pub(super) numerator: u32,
    pub(super) denominator: u32,
    pub(super) drop_frame: bool,
}

impl DeskTimecodeFrameRate {
    fn control_rate(self) -> Option<FrameRate> {
        if self.denominator == 1 {
            return u8::try_from(self.numerator)
                .ok()
                .and_then(FrameRate::whole_frames);
        }
        (self.numerator == 30_000 && self.denominator == 1_001 && self.drop_frame)
            .then_some(FrameRate::Fps2997Drop)
    }
}
impl Default for DeskConfiguration {
    fn default() -> Self {
        Self {
            frame_rate_hz: 44,
            output_bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            osc_bind: Some(SocketAddr::from(([127, 0, 0, 1], 9000))),
            art_timecode_bind: None,
            timecode_source: TimecodeSourceSelection::Internal,
            timecode_frame_rate: None,
            timecode_external_loss_policy: ExternalTimecodeLossPolicy::ContinueInternal,
            timecode_external_loss_timeout_millis: 500,
            osc_timecode: None,
            timecode_audio_output_device: None,
            timecode_audio_latency_trim_micros_by_output: BTreeMap::new(),
            backup_retention: 20,
            autosave_interval_seconds: default_autosave_interval_seconds(),
            speed_groups_bpm: default_speed_groups(),
            speed_group_sound_to_light: default_sound_to_light(),
            speed_group_sources: default_speed_group_sources(),
            programmer_fade_millis: 3_000,
            command_line_at_uses_programmer_fade: false,
            sequence_master_fade_millis: 3_000,
            cuelist_auto_off_at_zero_default: false,
            cuelist_auto_off_flash_release_default: false,
            start_after_first_recording: false,
            preload_programmer_changes: true,
            preload_physical_playback_actions: false,
            preload_virtual_playback_actions: true,
            patch_preview_highlight_dmx: false,
            highlight_look: light_fixture::HighlightLook::default(),
            highlight_legacy_overrides_acknowledged: false,
            matter_enabled: false,
            pool_presentation: PoolPresentationConfiguration::default(),
            update_settings_by_desk: HashMap::new(),
            file_manager_system_picker_fallback: false,
            file_manager_roots: Vec::new(),
            visualizer_views: BTreeMap::new(),
        }
    }
}
impl DeskConfiguration {
    pub(super) fn timecode_router_config(&self) -> TimecodeRouterConfig {
        TimecodeRouterConfig {
            selected_source: self.timecode_source.clone(),
            desk_rate: self
                .timecode_frame_rate
                .and_then(DeskTimecodeFrameRate::control_rate)
                .unwrap_or_else(|| {
                    FrameRate::whole_frames(u8::try_from(self.frame_rate_hz).unwrap_or(u8::MAX))
                        .expect("validated DMX frame rate is positive")
                }),
            external_loss_policy: self.timecode_external_loss_policy,
            loss_timeout_millis: self.timecode_external_loss_timeout_millis,
        }
    }

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

    /// An installation whose Highlight Look was only ever flagged for review, without an operator
    /// ever confirming legacy raw output, adopts the semantic look. Nothing is lost: opening a
    /// show that genuinely stores raw Highlight overrides raises the review requirement again.
    pub(super) fn migrate_highlight_look(&mut self) {
        if self.highlight_look.compatibility
            == light_fixture::HighlightLookCompatibility::NeedsReview
        {
            self.highlight_look.compatibility = light_fixture::HighlightLookCompatibility::Semantic;
        }
    }

    pub(super) fn validate(&self) -> Result<(), ApiError> {
        if !(40..=60).contains(&self.frame_rate_hz) {
            return Err(ApiError::bad_request("frame_rate_hz must be 40-60"));
        }
        if self.timecode_external_loss_timeout_millis == 0
            || self.timecode_external_loss_timeout_millis > 60_000
        {
            return Err(ApiError::bad_request(
                "timecode_external_loss_timeout_millis must be 1-60000",
            ));
        }
        if let TimecodeSourceSelection::External { source } = &self.timecode_source
            && source.trim().is_empty()
        {
            return Err(ApiError::bad_request(
                "timecode_source.external source must not be empty",
            ));
        }
        if self
            .timecode_frame_rate
            .is_some_and(|rate| rate.control_rate().is_none())
        {
            return Err(ApiError::bad_request(
                "timecode_frame_rate must be a positive whole-frame rate or 30000/1001 drop-frame",
            ));
        }
        if self
            .timecode_audio_output_device
            .as_ref()
            .is_some_and(|device| device.trim().is_empty())
        {
            return Err(ApiError::bad_request(
                "timecode_audio_output_device must be null or a non-empty exact device name",
            ));
        }
        if self
            .timecode_audio_latency_trim_micros_by_output
            .iter()
            .any(|(output, trim)| {
                output.trim().is_empty() || !(-5_000_000..=5_000_000).contains(trim)
            })
        {
            return Err(ApiError::bad_request(
                "Timecode audio output trim keys must be non-empty and values within +/-5000000 microseconds",
            ));
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
        for view in self.visualizer_views.values() {
            view.validate()?;
        }
        Ok(())
    }
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
    fn fresh_timing_and_preload_defaults_are_operator_safe() {
        let fresh = DeskConfiguration::default();
        assert!(!fresh.command_line_at_uses_programmer_fade);
        assert!(fresh.preload_programmer_changes);
        assert!(!fresh.preload_physical_playback_actions);
        assert!(fresh.preload_virtual_playback_actions);
    }

    #[test]
    fn explicit_persisted_timing_and_preload_choices_survive_default_changes() {
        let mut stored = serde_json::to_value(DeskConfiguration::default()).unwrap();
        let object = stored.as_object_mut().unwrap();
        object.insert("command_line_at_uses_programmer_fade".into(), true.into());
        object.insert("preload_programmer_changes".into(), false.into());
        object.insert("preload_physical_playback_actions".into(), true.into());
        object.insert("preload_virtual_playback_actions".into(), false.into());

        let decoded: DeskConfiguration = serde_json::from_value(stored).unwrap();
        assert!(decoded.command_line_at_uses_programmer_fade);
        assert!(!decoded.preload_programmer_changes);
        assert!(decoded.preload_physical_playback_actions);
        assert!(!decoded.preload_virtual_playback_actions);
    }

    #[test]
    fn playback_recording_defaults_are_disabled_for_fresh_and_legacy_installations() {
        let fresh = DeskConfiguration::default();
        assert!(!fresh.cuelist_auto_off_at_zero_default);
        assert!(!fresh.cuelist_auto_off_flash_release_default);
        assert!(!fresh.start_after_first_recording);

        let mut legacy = serde_json::to_value(&fresh).unwrap();
        let legacy_object = legacy.as_object_mut().unwrap();
        legacy_object.remove("cuelist_auto_off_at_zero_default");
        legacy_object.remove("cuelist_auto_off_flash_release_default");
        legacy_object.remove("start_after_first_recording");
        let decoded: DeskConfiguration = serde_json::from_value(legacy).unwrap();
        assert!(!decoded.cuelist_auto_off_at_zero_default);
        assert!(!decoded.cuelist_auto_off_flash_release_default);
        assert!(!decoded.start_after_first_recording);
    }

    #[test]
    fn fresh_and_legacy_configuration_use_the_semantic_white_default_look() {
        let fresh = DeskConfiguration::default();
        assert_eq!(
            fresh.highlight_look.compatibility,
            light_fixture::HighlightLookCompatibility::Semantic
        );
        assert_eq!(fresh.highlight_look.intensity, 1.0);
        assert_eq!(
            fresh.highlight_look.color,
            Some(light_fixture::HighlightColor::White)
        );

        let mut legacy = serde_json::to_value(DeskConfiguration::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("highlight_look");
        let decoded: DeskConfiguration = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.highlight_look, fresh.highlight_look);
    }

    #[test]
    fn an_unresolved_review_flag_migrates_to_the_semantic_look() {
        let mut configuration = DeskConfiguration::default();
        configuration.highlight_look.compatibility =
            light_fixture::HighlightLookCompatibility::NeedsReview;
        configuration.migrate_highlight_look();
        assert_eq!(
            configuration.highlight_look.compatibility,
            light_fixture::HighlightLookCompatibility::Semantic
        );

        // A desk that really is running the legacy raw path keeps it until the operator decides.
        configuration.highlight_look.compatibility =
            light_fixture::HighlightLookCompatibility::LegacyRaw;
        configuration.migrate_highlight_look();
        assert_eq!(
            configuration.highlight_look.compatibility,
            light_fixture::HighlightLookCompatibility::LegacyRaw
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

    #[test]
    fn production_output_rate_accepts_sixty_hertz_and_rejects_rates_above_it() {
        let mut configuration = DeskConfiguration {
            frame_rate_hz: 60,
            ..DeskConfiguration::default()
        };
        configuration.validate().unwrap();

        configuration.frame_rate_hz = 61;
        let error = configuration.validate().unwrap_err();
        assert_eq!(error.message, "frame_rate_hz must be 40-60");
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
