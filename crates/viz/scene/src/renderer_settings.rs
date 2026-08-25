//! Renderer settings shared by every operator surface and scene connection.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettings {
    pub source: String,
    pub host: String,
    pub port: u16,
    pub quality: Option<String>,
    pub fog: f32,
    pub persistence: f32,
    pub persistence_falloff: f32,
    pub ambient: f32,
    pub exposure: f32,
    pub laser_brightness: f32,
    pub lamp_fog_cloudiness: f32,
    pub lamp_fog_turbulence: f32,
    pub laser_fog_cloudiness: f32,
    pub laser_fog_turbulence: f32,
    pub crowd_amount: f32,
    pub theme: String,
    pub background: Option<[f32; 3]>,
    pub show_labels: bool,
    pub show_selection: bool,
    pub floor_grid: Option<bool>,
    pub blender: String,
    pub input_overrides: Vec<RendererInputOverride>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererInputOverride {
    pub universe: u16,
    pub protocol: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettingsUpdate {
    pub revision: u64,
    pub source: String,
    pub changed: Vec<String>,
    pub settings: RendererSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RendererSettingsIntent {
    pub request_id: String,
    pub source: String,
    pub changes: Vec<RendererSettingChange>,
}

/// One typed field mutation. Nullable settings carry `None` as their value rather than omitting
/// the change, so “Follow source” remains distinct from “quality was not edited”.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum RendererSettingChange {
    Source(String),
    Host(String),
    Port(u16),
    Quality(Option<String>),
    Fog(f32),
    Persistence(f32),
    PersistenceFalloff(f32),
    Ambient(f32),
    Exposure(f32),
    LaserBrightness(f32),
    LampFogCloudiness(f32),
    LampFogTurbulence(f32),
    LaserFogCloudiness(f32),
    LaserFogTurbulence(f32),
    CrowdAmount(f32),
    Theme(String),
    Background(Option<[f32; 3]>),
    ShowLabels(bool),
    ShowSelection(bool),
    FloorGrid(Option<bool>),
    Blender(String),
    InputOverrides(Vec<RendererInputOverride>),
}

impl Default for RendererSettings {
    fn default() -> Self {
        Self {
            source: "lighting_desk".into(),
            host: "127.0.0.1".into(),
            port: 5000,
            quality: None,
            fog: 0.15,
            persistence: 0.0,
            persistence_falloff: 3.0,
            ambient: 0.06,
            exposure: 1.0,
            laser_brightness: 1.0,
            lamp_fog_cloudiness: 0.7,
            lamp_fog_turbulence: 1.0,
            laser_fog_cloudiness: 0.0,
            laser_fog_turbulence: 0.0,
            crowd_amount: 1.0,
            theme: "light_on_dark".into(),
            background: None,
            show_labels: true,
            show_selection: true,
            floor_grid: None,
            blender: String::new(),
            input_overrides: Vec::new(),
        }
    }
}

impl RendererSettings {
    pub fn from_file(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once(char::is_whitespace)
                .map(|(key, value)| (key, value.trim()))
                .unwrap_or((line, ""));
            match key {
                "source" if matches!(value, "lighting_desk" | "planning_software") => {
                    settings.source = value.into()
                }
                "host" => settings.host = value.into(),
                "port" => settings.port = value.parse().unwrap_or(settings.port),
                "quality" => settings.quality = (value != "follow").then(|| value.into()),
                "fog" => adopt_f32(value, &mut settings.fog),
                "persistence" => adopt_f32(value, &mut settings.persistence),
                "persistence_falloff" => adopt_f32(value, &mut settings.persistence_falloff),
                "ambient" => adopt_f32(value, &mut settings.ambient),
                "exposure" => adopt_f32(value, &mut settings.exposure),
                "laser_brightness" => adopt_f32(value, &mut settings.laser_brightness),
                "lamp_fog_cloudiness" => adopt_f32(value, &mut settings.lamp_fog_cloudiness),
                "lamp_fog_turbulence" => adopt_f32(value, &mut settings.lamp_fog_turbulence),
                "laser_fog_cloudiness" => adopt_f32(value, &mut settings.laser_fog_cloudiness),
                "laser_fog_turbulence" => adopt_f32(value, &mut settings.laser_fog_turbulence),
                "crowd_amount" => adopt_f32(value, &mut settings.crowd_amount),
                "theme" if matches!(value, "light_on_dark" | "dark_on_light") => {
                    settings.theme = value.into()
                }
                "background" => settings.background = parse_background(value),
                "labels" => settings.show_labels = value != "false",
                "show_selection" => settings.show_selection = value != "false",
                "floor_grid" => settings.floor_grid = parse_optional_bool(value),
                "blender" => settings.blender = value.into(),
                "input" => {
                    if let Some((universe, protocol)) = value.split_once(char::is_whitespace)
                        && let Ok(universe) = universe.parse::<u16>()
                        && matches!(protocol.trim(), "artnet" | "sacn")
                    {
                        settings.input_overrides.push(RendererInputOverride {
                            universe,
                            protocol: protocol.trim().into(),
                        });
                    }
                }
                _ => {}
            }
        }
        settings
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("Visualizer server port must be from 1 to 65535".into());
        }
        if !matches!(
            self.quality.as_deref(),
            None | Some("draft" | "standard" | "high" | "ultra")
        ) {
            return Err(
                "Visualizer quality must be Follow source, Draft, Standard, High, or Ultra".into(),
            );
        }
        for (label, value, minimum, maximum) in [
            ("Fog amount", self.fog, 0.0, 1.0),
            ("Persistence", self.persistence, 0.0, 1.0),
            ("Persistence falloff", self.persistence_falloff, 1.0, 8.0),
            ("Environment brightness", self.ambient, 0.0, 1.0),
            ("Exposure", self.exposure, 0.05, 4.0),
            ("Laser brightness", self.laser_brightness, 0.0, 4.0),
            ("Lamp fog cloudiness", self.lamp_fog_cloudiness, 0.0, 1.0),
            ("Lamp fog turbulence", self.lamp_fog_turbulence, 0.0, 1.0),
            ("Laser fog cloudiness", self.laser_fog_cloudiness, 0.0, 1.0),
            ("Laser fog turbulence", self.laser_fog_turbulence, 0.0, 1.0),
            ("Crowd amount", self.crowd_amount, 0.0, 1.0),
        ] {
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(format!("{label} must be from {minimum} to {maximum}"));
            }
        }
        Ok(())
    }

    pub fn to_file(&self) -> String {
        let mut text = String::from("# ToskLight visualizer preferences\n");
        macro_rules! line {
            ($key:literal, $value:expr) => {{
                text.push_str(&format!(concat!($key, " {}\n"), $value));
            }};
        }
        line!("source", self.source);
        line!("host", self.host);
        line!("port", self.port);
        line!("quality", self.quality.as_deref().unwrap_or("follow"));
        line!("fog", self.fog);
        line!("persistence", self.persistence);
        line!("persistence_falloff", self.persistence_falloff);
        line!("ambient", self.ambient);
        line!("exposure", self.exposure);
        line!("laser_brightness", self.laser_brightness);
        line!("lamp_fog_cloudiness", self.lamp_fog_cloudiness);
        line!("lamp_fog_turbulence", self.lamp_fog_turbulence);
        line!("laser_fog_cloudiness", self.laser_fog_cloudiness);
        line!("laser_fog_turbulence", self.laser_fog_turbulence);
        line!("crowd_amount", self.crowd_amount);
        line!("theme", self.theme);
        match self.background {
            Some([red, green, blue]) => line!("background", format!("{red},{green},{blue}")),
            None => line!("background", "follow"),
        }
        line!("labels", self.show_labels);
        line!("show_selection", self.show_selection);
        line!(
            "floor_grid",
            self.floor_grid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "follow".into())
        );
        if !self.blender.trim().is_empty() {
            line!("blender", self.blender.trim());
        }
        for input in &self.input_overrides {
            line!("input", format!("{} {}", input.universe, input.protocol));
        }
        text
    }

    pub fn changes_from(&self, previous: &Self) -> Vec<RendererSettingChange> {
        let mut changes = Vec::new();
        macro_rules! field {
            ($field:ident, $variant:ident) => {
                if self.$field != previous.$field {
                    changes.push(RendererSettingChange::$variant(self.$field.clone()));
                }
            };
        }
        field!(source, Source);
        field!(host, Host);
        field!(port, Port);
        field!(quality, Quality);
        field!(fog, Fog);
        field!(persistence, Persistence);
        field!(persistence_falloff, PersistenceFalloff);
        field!(ambient, Ambient);
        field!(exposure, Exposure);
        field!(laser_brightness, LaserBrightness);
        field!(lamp_fog_cloudiness, LampFogCloudiness);
        field!(lamp_fog_turbulence, LampFogTurbulence);
        field!(laser_fog_cloudiness, LaserFogCloudiness);
        field!(laser_fog_turbulence, LaserFogTurbulence);
        field!(crowd_amount, CrowdAmount);
        field!(theme, Theme);
        field!(background, Background);
        field!(show_labels, ShowLabels);
        field!(show_selection, ShowSelection);
        field!(floor_grid, FloorGrid);
        field!(blender, Blender);
        field!(input_overrides, InputOverrides);
        changes
    }

    pub fn apply(&mut self, changes: &[RendererSettingChange]) {
        for change in changes {
            match change {
                RendererSettingChange::Source(value) => self.source = value.clone(),
                RendererSettingChange::Host(value) => self.host = value.clone(),
                RendererSettingChange::Port(value) => self.port = *value,
                RendererSettingChange::Quality(value) => self.quality = value.clone(),
                RendererSettingChange::Fog(value) => self.fog = *value,
                RendererSettingChange::Persistence(value) => self.persistence = *value,
                RendererSettingChange::PersistenceFalloff(value) => {
                    self.persistence_falloff = *value
                }
                RendererSettingChange::Ambient(value) => self.ambient = *value,
                RendererSettingChange::Exposure(value) => self.exposure = *value,
                RendererSettingChange::LaserBrightness(value) => self.laser_brightness = *value,
                RendererSettingChange::LampFogCloudiness(value) => {
                    self.lamp_fog_cloudiness = *value
                }
                RendererSettingChange::LampFogTurbulence(value) => {
                    self.lamp_fog_turbulence = *value
                }
                RendererSettingChange::LaserFogCloudiness(value) => {
                    self.laser_fog_cloudiness = *value
                }
                RendererSettingChange::LaserFogTurbulence(value) => {
                    self.laser_fog_turbulence = *value
                }
                RendererSettingChange::CrowdAmount(value) => self.crowd_amount = *value,
                RendererSettingChange::Theme(value) => self.theme = value.clone(),
                RendererSettingChange::Background(value) => self.background = *value,
                RendererSettingChange::ShowLabels(value) => self.show_labels = *value,
                RendererSettingChange::ShowSelection(value) => self.show_selection = *value,
                RendererSettingChange::FloorGrid(value) => self.floor_grid = *value,
                RendererSettingChange::Blender(value) => self.blender = value.clone(),
                RendererSettingChange::InputOverrides(value) => {
                    self.input_overrides = value.clone()
                }
            }
        }
    }
}

impl RendererSettingChange {
    pub fn field_name(&self) -> &'static str {
        match self {
            Self::Source(_) => "source",
            Self::Host(_) => "host",
            Self::Port(_) => "port",
            Self::Quality(_) => "quality",
            Self::Fog(_) => "fog",
            Self::Persistence(_) => "persistence",
            Self::PersistenceFalloff(_) => "persistenceFalloff",
            Self::Ambient(_) => "ambient",
            Self::Exposure(_) => "exposure",
            Self::LaserBrightness(_) => "laserBrightness",
            Self::LampFogCloudiness(_) => "lampFogCloudiness",
            Self::LampFogTurbulence(_) => "lampFogTurbulence",
            Self::LaserFogCloudiness(_) => "laserFogCloudiness",
            Self::LaserFogTurbulence(_) => "laserFogTurbulence",
            Self::CrowdAmount(_) => "crowdAmount",
            Self::Theme(_) => "theme",
            Self::Background(_) => "background",
            Self::ShowLabels(_) => "showLabels",
            Self::ShowSelection(_) => "showSelection",
            Self::FloorGrid(_) => "floorGrid",
            Self::Blender(_) => "blender",
            Self::InputOverrides(_) => "inputOverrides",
        }
    }
}

fn adopt_f32(value: &str, target: &mut f32) {
    if let Ok(value) = value.parse() {
        *target = value;
    }
}

fn parse_background(value: &str) -> Option<[f32; 3]> {
    if value == "follow" {
        return None;
    }
    let channels = value
        .split(',')
        .filter_map(|channel| channel.trim().parse::<f32>().ok())
        .collect::<Vec<_>>();
    match channels.as_slice() {
        [red, green, blue] => Some([*red, *green, *blue]),
        _ => None,
    }
}

fn parse_optional_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_the_persistence_contract() {
        let source = "source lighting_desk\nhost desk.local\nport 5001\
quality ultra\nfog 0.08\npersistence 0.12\npersistence_falloff 4\nambient 0.09\nexposure 1.2\nlaser_brightness 1.5\nlamp_fog_cloudiness 0.2\nlamp_fog_turbulence 0.3\nlaser_fog_cloudiness 0.4\nlaser_fog_turbulence 0.5\ncrowd_amount 0.75\ntheme dark_on_light\nbackground 0.1,0.2,0.3\nlabels false\nshow_selection true\nfloor_grid false\nblender /Applications/Blender.app\ninput 2 sacn\n";
        let settings = RendererSettings::from_file(source);
        settings.validate().unwrap();
        assert_eq!(RendererSettings::from_file(&settings.to_file()), settings);
    }

    #[test]
    fn changed_fields_name_only_the_values_that_moved() {
        let previous = RendererSettings::default();
        let settings = RendererSettings {
            quality: Some("draft".into()),
            fog: 0.02,
            ..previous.clone()
        };
        assert_eq!(
            settings
                .changes_from(&previous)
                .iter()
                .map(RendererSettingChange::field_name)
                .collect::<Vec<_>>(),
            ["quality", "fog"]
        );
    }
}
