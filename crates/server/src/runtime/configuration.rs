use super::*;

pub(super) fn default_speed_groups() -> [f64; 5] {
    [120.0, 90.0, 60.0, 30.0, 15.0]
}

pub(super) fn default_sound_to_light() -> [SoundToLightConfig; 5] {
    std::array::from_fn(|_| SoundToLightConfig::default())
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
    #[serde(
        default = "default_speed_groups",
        deserialize_with = "deserialize_speed_groups"
    )]
    pub(super) speed_groups_bpm: [f64; 5],
    #[serde(default = "default_sound_to_light")]
    pub(super) speed_group_sound_to_light: [SoundToLightConfig; 5],
    pub(super) programmer_fade_millis: u64,
    pub(super) sequence_master_fade_millis: u64,
    pub(super) preload_programmer_changes: bool,
    pub(super) preload_physical_playback_actions: bool,
    pub(super) preload_virtual_playback_actions: bool,
    /// Allow Show Patch's scoped Stage preview selection to identify fixtures on DMX.
    pub(super) patch_preview_highlight_dmx: bool,
    /// Desk-persistent opt-in for the global page/playback Matter bridge.
    pub(super) matter_enabled: bool,
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
            speed_groups_bpm: default_speed_groups(),
            speed_group_sound_to_light: default_sound_to_light(),
            programmer_fade_millis: 3_000,
            sequence_master_fade_millis: 3_000,
            preload_programmer_changes: true,
            preload_physical_playback_actions: true,
            preload_virtual_playback_actions: false,
            patch_preview_highlight_dmx: false,
            matter_enabled: false,
            update_settings_by_desk: HashMap::new(),
            file_manager_system_picker_fallback: false,
            file_manager_roots: Vec::new(),
        }
    }
}
impl DeskConfiguration {
    pub(super) fn validate(&self) -> Result<(), ApiError> {
        if !(40..=44).contains(&self.frame_rate_hz) {
            return Err(ApiError::bad_request("frame_rate_hz must be 40-44"));
        }
        if self.backup_retention == 0 || self.backup_retention > 1_000 {
            return Err(ApiError::bad_request("backup_retention must be 1-1000"));
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
        if self.programmer_fade_millis > 60_000 || self.sequence_master_fade_millis > 60_000 {
            return Err(ApiError::bad_request(
                "fade times must be 0-60000 milliseconds",
            ));
        }
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
        Ok(())
    }
}
