//! Command-line options, renderer-local preferences, and the Quick Settings model.
//!
//! Renderer-local preferences — the haze amount and an explicit quality override — are
//! deliberately stored next to the application, never in the show or planning document.

use std::path::PathBuf;
use viz_scene::{AtmospherePreference, PersistencePreference, ProviderKind, RenderQuality, Theme};

pub const DEFAULT_DESK_HOST: &str = "127.0.0.1";
pub const DEFAULT_DESK_PORT: u16 = 5000;
/// The future planning provider gets its own documented default rather than assuming the desk
/// port.
pub const DEFAULT_PLANNER_PORT: u16 = 5310;

/// What the visualizer was asked to look at.
///
/// The three ways it starts are genuinely different products of the same window: attached to a
/// console, pointed at a file, or opened on its own with nothing chosen yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Startup {
    /// A lighting desk was named, or the desk started this itself. Just visualize.
    Desk,
    /// A show file was named. Serve it and visualize it; no planning window.
    Show(PathBuf),
    /// Nothing was named. Open the planning window so the operator can choose or build a rig.
    Planning,
    /// The built-in deterministic scene.
    Demo,
    /// Started by the desk as a supervised helper. The scene, values and view arrive over the
    /// private channel on stdin rather than from a desk over HTTP, and this process draws them
    /// and nothing else.
    Helper,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub help: bool,
    pub host: String,
    pub port: u16,
    /// Set when the operator or the desk named a connection, which distinguishes "connect to this
    /// desk" from "nothing was asked for".
    pub desk_requested: bool,
    /// This launch was given an already-running Viz editor scene source. It uses the same HTTP
    /// provider protocol as a desk, but must neither open another planning window nor inherit a
    /// stored desk source.
    pub planning_server_requested: bool,
    /// Set when this launch named the desk user, so stored preferences do not override it.
    /// Which renderer this window is, for a desk driving more than one. The desk keeps one view
    /// per target, so two windows can be told to show two different things.
    pub target: String,
    pub source: ProviderKind,
    /// Start with the built-in deterministic scene instead of connecting. Used by the
    /// provider-boundary tests and for offline renderer inspection.
    pub demo: bool,
    /// Present one frame and exit, writing nothing. Used by smoke tests on build machines.
    pub verify_only: bool,
    /// Render `--capture-frames` frames, write a PNG here, and exit.
    pub view: Option<viz_scene::ViewMode>,
    pub quality: Option<RenderQuality>,
    /// Starting look, for an operator who wants a fixed one and for evidence captures.
    pub theme: Option<Theme>,
    pub ambient: Option<f32>,
    pub exposure: Option<f32>,
    /// What lasers are drawn at, `1.0` being the built-in strength.
    pub laser_brightness: Option<f32>,
    /// Fraction of every authored crowd to draw in this window.
    pub crowd_amount: Option<f32>,
    /// Zoom applied once after the scene is framed, for close-up evidence captures.
    pub zoom: Option<f32>,
    /// Haze amount to start with. The renderer's own haze setting, never taken from the show.
    pub fog: Option<f32>,
    pub capture: Option<PathBuf>,
    pub capture_frames: u32,
    /// Run for this many seconds, then print a performance report and exit.
    pub benchmark_seconds: Option<f32>,
    /// Cycle every named view during a benchmark run.
    pub benchmark_all_views: bool,
    pub preferences_path: Option<PathBuf>,
    /// Open this show file on startup instead of connecting to a running desk.
    pub show: Option<PathBuf>,
    /// The Blender to export snapshots with, for a machine with more than one.
    pub blender: Option<PathBuf>,
    /// A directory of loose laser scan scripts that override the ones fixture packages ship.
    /// This is the authoring loop: edit `<profile id>.js` and the change is picked up live.
    pub laser_scripts: Option<PathBuf>,
    /// Take one snapshot once the scene has settled, print where it went, and exit.
    pub snapshot: bool,
    /// Run as the desk's supervised renderer helper.
    pub helper: bool,
    /// Draw the desk's Stage pane with no window of this process's own.
    pub embed: bool,
}

/// The strongest lasers may be drawn at.
///
/// Four times the built-in strength, which is well past subtle and into "show me where they are"
/// — the point of the range is that an operator can find the setting that suits their room, not
/// that any part of it is realistic.
pub const MAX_LASER_BRIGHTNESS: f32 = 4.0;

/// Read a `0..=100` percentage argument as a `0..=1` fraction.
fn percent(arguments: &mut impl Iterator<Item = String>, flag: &str) -> Result<f32, String> {
    bounded_percent(arguments, flag, 1.0)
}

/// The same, for a setting whose hundred per cent is not its maximum.
fn bounded_percent(
    arguments: &mut impl Iterator<Item = String>,
    flag: &str,
    most: f32,
) -> Result<f32, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{flag} needs a percentage"))?;
    let parsed = value
        .trim_end_matches('%')
        .parse::<f32>()
        .map_err(|_| format!("{flag} needs a percentage, not \"{value}\""))?;
    Ok((parsed / 100.0).clamp(0.0, most))
}

/// The flags that decide how the picture looks, kept apart from the ones that decide what it is
/// looking at. Returns `false` when the flag is none of these, so the caller can go on trying.
fn parse_look_flag(
    flag: &str,
    arguments: &mut impl Iterator<Item = String>,
    options: &mut Options,
) -> Result<bool, String> {
    match flag {
        "--view" => {
            let value = arguments
                .next()
                .ok_or_else(|| "--view needs a named view".to_owned())?;
            options.view = Some(
                viz_scene::ViewMode::from_wire(&value)
                    .ok_or_else(|| format!("unknown view \"{value}\""))?,
            );
        }
        "--quality" => {
            let value = arguments
                .next()
                .ok_or_else(|| "--quality needs a quality tier".to_owned())?;
            options.quality = Some(
                RenderQuality::from_wire(&value)
                    .ok_or_else(|| format!("unknown quality \"{value}\""))?,
            );
        }
        "--theme" => {
            let value = arguments
                .next()
                .ok_or_else(|| "--theme needs light_on_dark or dark_on_light".to_owned())?;
            options.theme =
                Some(Theme::from_wire(&value).ok_or_else(|| format!("unknown theme \"{value}\""))?);
        }
        "--ambient" => options.ambient = Some(percent(&mut *arguments, "--ambient")?),
        "--zoom" => {
            let value = arguments
                .next()
                .ok_or_else(|| "--zoom needs a factor".to_owned())?;
            options.zoom = Some(
                value
                    .parse::<f32>()
                    .map_err(|_| format!("--zoom needs a number, not \"{value}\""))?
                    .clamp(0.02, 20.0),
            );
        }
        "--fog" => options.fog = Some(percent(&mut *arguments, "--fog")?),
        "--laser" => {
            options.laser_brightness = Some(bounded_percent(
                &mut *arguments,
                "--laser",
                MAX_LASER_BRIGHTNESS,
            )?);
        }
        "--crowd" => options.crowd_amount = Some(percent(&mut *arguments, "--crowd")?),
        "--exposure" => {
            let value = arguments
                .next()
                .ok_or_else(|| "--exposure needs a multiplier".to_owned())?;
            options.exposure = Some(
                value
                    .parse::<f32>()
                    .map_err(|_| format!("--exposure needs a number, not \"{value}\""))?
                    .clamp(0.05, 4.0),
            );
        }
        _ => return Ok(false),
    }
    Ok(true)
}

impl Default for Options {
    fn default() -> Self {
        Self {
            help: false,
            laser_scripts: None,
            host: DEFAULT_DESK_HOST.to_owned(),
            port: DEFAULT_DESK_PORT,
            desk_requested: false,
            planning_server_requested: false,
            target: "main".to_owned(),
            source: ProviderKind::LightingDesk,
            demo: false,
            verify_only: false,
            view: None,
            quality: None,
            theme: None,
            ambient: None,
            exposure: None,
            laser_brightness: None,
            crowd_amount: None,
            zoom: None,
            fog: None,
            capture: None,
            capture_frames: 60,
            benchmark_seconds: None,
            benchmark_all_views: false,
            preferences_path: None,
            show: None,
            blender: None,
            snapshot: false,
            helper: false,
            embed: false,
        }
    }
}

impl Options {
    pub fn usage() -> &'static str {
        concat!(
            "ToskLight visualizer\n",
            "\n",
            "Usage: viz-renderer [options]\n",
            "\n",
            "  --server <host>   Lighting-desk host or IP address (default 127.0.0.1)\n",
            "  --planning-server <host>  Existing Viz editor scene source\n",
            "  --port <1-65535>  Lighting-desk API port (default 5000)\n",
            "  --target <name>   Which renderer the desk addresses (default main)\n",
            "  --demo            Open the same canonical Demo Show shipped with Desk and Editor\n",
            "  --verify          Open the window, present one frame, and exit\n",
            "  --capture <path>  Write one rendered PNG and exit\n",
            "  --capture-frames  Frames to settle before capturing (default 60)\n",
            "  --view <name>     Named view, for example top_down or full_3d\n",
            "  --quality <tier>  draft | standard | high | ultra\n",
            "  --show <path>     Open this show file instead of connecting to a running desk\n",
            "  --blender <path>  Blender to export snapshots with (found automatically otherwise)\n",
            "  --laser-scripts <dir>  Laser scan scripts overriding the ones fixtures ship\n",
            "  --snapshot        Take one snapshot once the scene settles, then exit\n",
            "  --helper          Run as the desk's supervised renderer helper\n",
            "  --theme <name>    light_on_dark | dark_on_light\n",
            "  --ambient <pct>   Brightness of everything that is not a light source\n",
            "  --fog <pct>       Haze amount to render with (default 15)\n",
            "  --exposure <x>    Operator exposure trim, 0.05-4.0\n",
            "  --laser <pct>     Brightness of every laser, 0-400 (default 100)\n",
            "  --crowd <pct>    Fraction of authored audiences to draw (default 100)\n",
            "  --zoom <factor>   Zoom in (<1) or out (>1) once the scene is framed\n",
            "  --benchmark <s>   Measure for this many seconds, print a report, and exit\n",
            "  --benchmark-all-views  Cycle every named view during the measurement\n",
            "  --preferences <path>  Keep this window's settings here instead of the default\n",
            "  --help            Show this message\n",
            "\n",
            "With no --server, --show or --demo, the planning window opens so a rig can be\n",
            "chosen or built. A desk that launches the visualizer names itself and is visualized\n",
            "directly.\n",
            "\n",
            "Enter opens Quick Settings. 1-8 pick a view, Space hides the overlays,\n",
            "T switches light on dark and dark on light, L switches the plan labels.\n",
        )
    }

    /// How this launch was asked to find a scene.
    ///
    /// A desk that launches the visualizer names itself, so an explicit connection always wins. A
    /// bare launch is somebody opening the application on its own, with no console and no file in
    /// mind: that is the planning window's case, not an empty picture.
    pub fn startup(&self) -> Startup {
        self.startup_with_recent_show(viz_document::standalone::recent_show(), desk_launched())
    }

    /// Resolve launch precedence from already-discovered machine state.
    ///
    /// Keeping discovery outside the decision makes the ordering testable without reading the
    /// operator's real recent-show record. Native launch verification deliberately creates that
    /// record, and a unit test must have the same answer before and after the product is exercised.
    fn startup_with_recent_show(
        &self,
        recent_show: Option<PathBuf>,
        launched_by_desk: bool,
    ) -> Startup {
        // The desk started this and owns it completely: it says what to draw over the channel, so
        // nothing else — not a remembered document, not the planning window — may take over.
        if self.helper {
            return Startup::Helper;
        }
        if self.demo {
            return Startup::Demo;
        }
        if let Some(path) = self.show.as_ref() {
            return Startup::Show(path.clone());
        }
        if self.desk_requested || launched_by_desk {
            return Startup::Desk;
        }
        // Nothing was named, so this is somebody opening the product on its own. Show them the rig
        // they were last looking at rather than an empty picture: one application means the
        // document the editor had open is this launch's document too. A record naming a show that
        // has since been moved or deleted is no record at all, and falls through to the editor.
        if let Some(recent) = recent_show {
            return Startup::Show(recent);
        }
        Startup::Planning
    }

    pub fn from_arguments(arguments: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--help" | "-h" => options.help = true,
                "--demo" => options.demo = true,
                "--snapshot" => options.snapshot = true,
                "--helper" => options.helper = true,
                // Implies `--helper`: an embedded pane is by definition the desk's, and naming it
                // alone would otherwise start a standalone visualizer with nowhere to send frames.
                "--embed" => {
                    options.helper = true;
                    options.embed = true;
                }
                "--verify" => options.verify_only = true,
                "--capture" => {
                    options.capture =
                        Some(PathBuf::from(arguments.next().ok_or_else(|| {
                            "--capture needs an output PNG path".to_owned()
                        })?));
                }
                "--benchmark" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--benchmark needs a duration in seconds".to_owned())?;
                    options.benchmark_seconds =
                        Some(value.parse().map_err(|_| {
                            format!("--benchmark must be seconds, not \"{value}\"")
                        })?);
                }
                "--benchmark-all-views" => options.benchmark_all_views = true,
                "--capture-frames" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--capture-frames needs a count".to_owned())?;
                    options.capture_frames = value.parse().map_err(|_| {
                        format!("--capture-frames must be a number, not \"{value}\"")
                    })?;
                }
                flag if parse_look_flag(flag, &mut arguments, &mut options)? => {}
                "--show" => {
                    options.show = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--show needs a show file path".to_owned())?
                            .into(),
                    );
                }
                "--laser-scripts" => {
                    options.laser_scripts = Some(
                        arguments
                            .next()
                            .ok_or_else(|| {
                                "--laser-scripts needs a directory of scan scripts".to_owned()
                            })?
                            .into(),
                    );
                }
                "--blender" => {
                    options.blender = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--blender needs a path to Blender".to_owned())?
                            .into(),
                    );
                }
                "--server" => {
                    options.host = arguments
                        .next()
                        .ok_or_else(|| "--server needs a host or IP address".to_owned())?;
                    options.desk_requested = true;
                }
                "--planning-server" => {
                    options.host = arguments
                        .next()
                        .ok_or_else(|| "--planning-server needs a host or IP address".to_owned())?;
                    options.source = ProviderKind::PlanningSoftware;
                    options.desk_requested = true;
                    options.planning_server_requested = true;
                }
                "--target" => {
                    options.target = arguments
                        .next()
                        .ok_or_else(|| "--target needs a renderer name".to_owned())?
                        .to_ascii_lowercase();
                }
                "--preferences" => {
                    options.preferences_path = Some(
                        arguments
                            .next()
                            .ok_or_else(|| "--preferences needs a file path".to_owned())?
                            .into(),
                    );
                }
                "--port" => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--port needs a number from 1 to 65535".to_owned())?;
                    options.port = parse_port(&value)?;
                    options.desk_requested = true;
                }
                other => return Err(format!("unknown option {other}")),
            }
        }
        Ok(options)
    }
}

/// The desk sets this when it launches the visualizer itself, so a desk-owned window never opens
/// a planning surface the operator did not ask for.
pub fn desk_launched() -> bool {
    std::env::var("TOSKLIGHT_VIZ_LAUNCHED_BY").is_ok_and(|value| value == "desk")
}

/// Where this launch keeps its preferences, or `None` when there is nowhere to keep them.
///
/// Beside the operator's own application data, never in the repository, in a show, or in a
/// planning document: these are settings of this window on this machine.
pub fn preferences_path(options: &Options) -> Option<PathBuf> {
    if let Some(path) = options.preferences_path.as_ref() {
        return Some(path.clone());
    }
    if let Some(path) = std::env::var_os(PREFERENCES_PATH_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|home| {
            home.join("Library/Application Support/ToskLight/Visualizer/preferences.conf")
        });
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA")
            .map(|data| PathBuf::from(data).join("ToskLight/Visualizer/preferences.conf"));
    }
    if let Some(config) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(config).join("tosklight/visualizer/preferences.conf"));
    }
    home.map(|home| home.join(".config/tosklight/visualizer/preferences.conf"))
}

/// Points the preferences file somewhere else, which is how a test keeps out of the operator's own
/// configuration.
pub const PREFERENCES_PATH_ENV: &str = "TOSKLIGHT_VIZ_PREFERENCES";

/// Write `preferences` where this launch keeps them, creating the folder if it has to.
pub fn store_preferences(path: &std::path::Path, preferences: &Preferences) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, preferences.to_file())
}

/// Ports are validated as `1..=65535`, matching the Quick Settings contract.
pub fn parse_port(value: &str) -> Result<u16, String> {
    match value.trim().parse::<u32>() {
        Ok(port) if (1..=65_535).contains(&port) => Ok(port as u16),
        _ => Err(format!("port must be from 1 to 65535, not \"{value}\"")),
    }
}

fn adopt_unit(value: &str, target: &mut f32) {
    if let Ok(parsed) = value.parse::<f32>() {
        *target = parsed.clamp(0.0, 1.0);
    }
}

/// Renderer-local preferences. Never written back to the connected source.
#[derive(Clone, Debug)]
pub struct Preferences {
    pub source: ProviderKind,
    pub host: String,
    pub port: u16,
    /// `None` follows the source's quality; `Some` overrides it locally.
    pub quality_override: Option<RenderQuality>,
    pub atmosphere: AtmospherePreference,
    /// How long a light goes on being seen after it has gone dark. Renderer-local for exactly the
    /// reason the haze amount is: it describes the observer, not the rig.
    pub persistence: PersistencePreference,
    /// How brightly everything that is not a light source is lit, `0..=1`.
    pub ambient: f32,
    /// Operator exposure trim, multiplied onto the renderer's automatic exposure.
    pub exposure: f32,
    /// What every laser in the rig is drawn at, `1.0` being the built-in strength. Renderer-local
    /// for the same reason the haze is: how strong a laser looks is a property of the room and the
    /// eye, not of the show.
    pub laser_brightness: f32,
    /// Ultra-only fog character. These remain renderer-local and never rewrite a show.
    pub fog_variation: viz_scene::FogVariation,
    /// Local audience amount; it never rewrites a Venue fixture or its deterministic seed.
    pub crowd_amount: f32,
    pub theme: Theme,
    /// The room colour behind the rig.
    pub background: Option<[f32; 3]>,
    /// Show screen-space fixture numbers and patch addresses in every Stage view.
    pub show_labels: bool,
    /// Draw selection emphasis received from the source.
    pub show_selection: bool,
    /// Draw the renderer's floor grid in spatial views.
    pub floor_grid: Option<bool>,
    /// Hide every overlay so the picture can be looked at on its own.
    ///
    /// Not kept between launches, and deliberately: a window that opens with no status surface,
    /// no connection state and nothing on screen is indistinguishable from a broken one. Pressing
    /// `Space` is a gesture for looking at the picture now, not a setting.
    pub overlays_hidden: bool,
    /// Where each overridden universe actually arrives. A universe with no entry follows the
    /// show's own output routes.
    pub input_overrides: Vec<viz_dmx::UniverseInput>,
    /// The Blender to export snapshots with. Empty lets the application find one, which is what
    /// an ordinary installation needs; naming one is for a machine with several.
    pub blender: String,
}

impl Preferences {
    pub fn from_options(options: &Options) -> Self {
        Self {
            source: options.source,
            host: options.host.clone(),
            port: options.port,
            quality_override: options.quality,
            atmosphere: match options.fog {
                Some(amount) => AtmospherePreference { amount },
                None => AtmospherePreference::default(),
            },
            persistence: PersistencePreference::default(),
            ambient: options.ambient.unwrap_or(0.06),
            exposure: options.exposure.unwrap_or(1.0),
            laser_brightness: options.laser_brightness.unwrap_or(1.0),
            fog_variation: viz_scene::FogVariation::default(),
            crowd_amount: options.crowd_amount.unwrap_or(1.0),
            theme: options.theme.unwrap_or(Theme::LightOnDark),
            background: None,
            show_labels: true,
            show_selection: true,
            floor_grid: None,
            overlays_hidden: false,
            input_overrides: Vec::new(),
            blender: options
                .blender
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        }
    }

    /// The Blender the operator named, or `None` to let the application find one.
    pub fn blender_path(&self) -> Option<PathBuf> {
        let trimmed = self.blender.trim();
        (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
    }

    /// The protocol the operator pinned for `universe`, or `None` to follow the show.
    pub fn input_for(&self, universe: u16) -> Option<viz_dmx::Protocol> {
        self.input_overrides
            .iter()
            .find(|input| input.universe == universe)
            .map(|input| input.protocol)
    }

    /// Pin `universe` to a protocol, or clear it back to following the show.
    pub fn set_input(&mut self, universe: u16, protocol: Option<viz_dmx::Protocol>) {
        self.input_overrides
            .retain(|input| input.universe != universe);
        if let Some(protocol) = protocol {
            self.input_overrides
                .push(viz_dmx::UniverseInput::new(universe, protocol));
        }
        self.input_overrides.sort_by_key(|input| input.universe);
    }

    /// The preferences as they are written to disk.
    ///
    /// A plain `key value` list rather than a serialised structure: it is a handful of settings an
    /// operator may well want to read or repair in a text editor, and a file it cannot parse must
    /// never stop the visualizer opening.
    pub fn to_file(&self) -> String {
        let mut text = String::from("# ToskLight visualizer preferences\n");
        text.push_str(&format!("source {}\n", self.source.wire()));
        text.push_str(&format!("host {}\n", self.host));
        text.push_str(&format!("port {}\n", self.port));
        text.push_str(&format!(
            "quality {}\n",
            match self.quality_override {
                Some(quality) => quality.wire(),
                None => "follow",
            }
        ));
        text.push_str(&format!("fog {}\n", self.atmosphere.amount));
        text.push_str(&format!("persistence {}\n", self.persistence.decay_seconds));
        text.push_str(&format!(
            "persistence_falloff {}\n",
            self.persistence.falloff
        ));
        text.push_str(&format!("ambient {}\n", self.ambient));
        text.push_str(&format!("exposure {}\n", self.exposure));
        text.push_str(&format!("laser_brightness {}\n", self.laser_brightness));
        text.push_str(&format!(
            "lamp_fog_cloudiness {}\n",
            self.fog_variation.lamp_cloudiness
        ));
        text.push_str(&format!(
            "lamp_fog_turbulence {}\n",
            self.fog_variation.lamp_turbulence
        ));
        text.push_str(&format!(
            "laser_fog_cloudiness {}\n",
            self.fog_variation.laser_cloudiness
        ));
        text.push_str(&format!(
            "laser_fog_turbulence {}\n",
            self.fog_variation.laser_turbulence
        ));
        text.push_str(&format!("crowd_amount {}\n", self.crowd_amount));
        text.push_str(&format!("theme {}\n", self.theme.wire()));
        match self.background {
            Some(background) => text.push_str(&format!(
                "background {},{},{}\n",
                background[0], background[1], background[2]
            )),
            None => text.push_str("background follow\n"),
        }
        text.push_str(&format!("labels {}\n", self.show_labels));
        text.push_str(&format!("show_selection {}\n", self.show_selection));
        text.push_str(&format!(
            "floor_grid {}\n",
            self.floor_grid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "follow".into())
        ));
        if !self.blender.trim().is_empty() {
            text.push_str(&format!("blender {}\n", self.blender.trim()));
        }
        for input in &self.input_overrides {
            text.push_str(&format!(
                "input {} {}\n",
                input.universe,
                input.protocol.wire()
            ));
        }
        text
    }

    pub fn renderer_settings(&self) -> viz_scene::RendererSettings {
        viz_scene::RendererSettings {
            source: self.source.wire().into(),
            host: self.host.clone(),
            port: self.port,
            quality: self.quality_override.map(|quality| quality.wire().into()),
            fog: self.atmosphere.amount,
            persistence: self.persistence.decay_seconds,
            persistence_falloff: self.persistence.falloff,
            ambient: self.ambient,
            exposure: self.exposure,
            laser_brightness: self.laser_brightness,
            lamp_fog_cloudiness: self.fog_variation.lamp_cloudiness,
            lamp_fog_turbulence: self.fog_variation.lamp_turbulence,
            laser_fog_cloudiness: self.fog_variation.laser_cloudiness,
            laser_fog_turbulence: self.fog_variation.laser_turbulence,
            crowd_amount: self.crowd_amount,
            theme: self.theme.wire().into(),
            background: self.background,
            show_labels: self.show_labels,
            show_selection: self.show_selection,
            floor_grid: self.floor_grid,
            blender: self.blender.clone(),
            input_overrides: self
                .input_overrides
                .iter()
                .map(|input| viz_scene::RendererInputOverride {
                    universe: input.universe,
                    protocol: input.protocol.wire().into(),
                })
                .collect(),
        }
    }

    /// Adopt stored preferences, keeping anything this launch named on the command line.
    ///
    /// An option given on the command line is what the operator asked for now, so it wins over
    /// what they last left the window set to. A line this build does not understand is skipped:
    /// preferences are a convenience, never a reason to refuse to start.
    pub fn adopt_file(&mut self, text: &str, options: &Options) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = match line.split_once(char::is_whitespace) {
                Some((key, value)) => (key, value.trim()),
                None => (line, ""),
            };
            match key {
                // A launch that named what to look at has already chosen its source; only a
                // launch that named nothing goes back to the one the operator left selected.
                "source"
                    if !options.helper
                        && !options.demo
                        && options.show.is_none()
                        && !options.desk_requested
                        && !desk_launched() =>
                {
                    if let Some(source) = ProviderKind::from_wire(value) {
                        self.source = source;
                    }
                }
                "host" if !options.desk_requested => self.host = value.to_owned(),
                "port" if !options.desk_requested => {
                    if let Ok(port) = parse_port(value) {
                        self.port = port;
                    }
                }
                "quality" if options.quality.is_none() => {
                    self.quality_override = RenderQuality::from_wire(value);
                }
                "fog" if options.fog.is_none() => {
                    if let Ok(amount) = value.parse::<f32>() {
                        self.atmosphere.amount = amount.clamp(0.0, 1.0);
                    }
                }
                "persistence" => {
                    if let Ok(seconds) = value.parse::<f32>() {
                        self.persistence.decay_seconds = seconds.clamp(0.0, 1.0);
                    }
                }
                "persistence_falloff" => {
                    if let Ok(falloff) = value.parse::<f32>() {
                        self.persistence.falloff = falloff.clamp(1.0, 8.0);
                    }
                }
                "ambient" if options.ambient.is_none() => {
                    if let Ok(ambient) = value.parse::<f32>() {
                        self.ambient = ambient.clamp(0.0, 1.0);
                    }
                }
                "exposure" if options.exposure.is_none() => {
                    if let Ok(exposure) = value.parse::<f32>() {
                        self.exposure = exposure.clamp(0.05, 4.0);
                    }
                }
                "laser_brightness" if options.laser_brightness.is_none() => {
                    if let Ok(brightness) = value.parse::<f32>() {
                        self.laser_brightness = brightness.clamp(0.0, MAX_LASER_BRIGHTNESS);
                    }
                }
                "lamp_fog_cloudiness" => adopt_unit(value, &mut self.fog_variation.lamp_cloudiness),
                "lamp_fog_turbulence" => adopt_unit(value, &mut self.fog_variation.lamp_turbulence),
                "laser_fog_cloudiness" => {
                    adopt_unit(value, &mut self.fog_variation.laser_cloudiness)
                }
                "laser_fog_turbulence" => {
                    adopt_unit(value, &mut self.fog_variation.laser_turbulence)
                }
                "crowd_amount" if options.crowd_amount.is_none() => {
                    if let Ok(amount) = value.parse::<f32>() {
                        self.crowd_amount = amount.clamp(0.0, 1.0);
                    }
                }
                "theme" if options.theme.is_none() => {
                    if let Some(theme) = Theme::from_wire(value) {
                        self.theme = theme;
                    }
                }
                "background" => {
                    if value == "follow" {
                        self.background = None;
                        continue;
                    }
                    let channels: Vec<f32> = value
                        .split(',')
                        .filter_map(|channel| channel.trim().parse::<f32>().ok())
                        .collect();
                    if let [red, green, blue] = channels.as_slice() {
                        self.background = Some([
                            red.clamp(0.0, 1.0),
                            green.clamp(0.0, 1.0),
                            blue.clamp(0.0, 1.0),
                        ]);
                    }
                }
                "labels" => self.show_labels = value != "false",
                "show_selection" => self.show_selection = value != "false",
                "floor_grid" => {
                    self.floor_grid = match value {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    }
                }
                // Written by an earlier version, and deliberately not read: see the field.
                "overlays_hidden" => {}
                "blender" if options.blender.is_none() => self.blender = value.to_owned(),
                "input" => {
                    if let Some((universe, protocol)) = value.split_once(char::is_whitespace)
                        && let Ok(universe) = universe.trim().parse::<u16>()
                        && let Some(protocol) = viz_dmx::Protocol::from_wire(protocol.trim())
                    {
                        self.set_input(universe, Some(protocol));
                    }
                }
                _ => {}
            }
        }
    }

    /// Build the preferences this launch starts with: the stored ones, then the command line.
    pub fn restored(options: &Options) -> Self {
        let mut preferences = Self::from_options(options);
        if let Some(path) = preferences_path(options)
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            preferences.adopt_file(&text, options);
        }
        preferences
    }

    pub fn quality_label(&self) -> String {
        match self.quality_override {
            None => "Follow source".to_owned(),
            Some(quality) => quality.label().to_owned(),
        }
    }
}

/// One staged edit of the connection fields. Cancelling leaves the live connection untouched.
#[derive(Clone, Debug)]
pub struct StagedConnection {
    pub source: ProviderKind,
    pub host: String,
    pub port_text: String,
}

impl StagedConnection {
    pub fn from_preferences(preferences: &Preferences) -> Self {
        Self {
            source: preferences.source,
            host: preferences.host.clone(),
            port_text: preferences.port.to_string(),
        }
    }

    pub fn validate(&self) -> Result<(String, u16), String> {
        if self.host.trim().is_empty() {
            return Err("Server must be a hostname or IP address".into());
        }
        let port = parse_port(&self.port_text)?;
        Ok((self.host.trim().to_owned(), port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn the_default_connection_is_the_local_desk() {
        let options = Options::from_arguments(arguments(&[])).unwrap();
        assert_eq!(options.host, "127.0.0.1");
        assert_eq!(options.port, 5000);
        assert_eq!(options.source, ProviderKind::LightingDesk);
    }

    #[test]
    fn a_remote_desk_address_is_accepted() {
        let options =
            Options::from_arguments(arguments(&["--server", "10.0.0.9", "--port", "5100"]))
                .unwrap();
        assert_eq!(options.host, "10.0.0.9");
        assert_eq!(options.port, 5100);
    }

    #[test]
    fn an_existing_planning_server_is_used_without_opening_another_editor() {
        let options = Options::from_arguments(arguments(&[
            "--planning-server",
            "127.0.0.1",
            "--port",
            "5311",
        ]))
        .unwrap();
        assert_eq!(options.host, "127.0.0.1");
        assert_eq!(options.port, 5311);
        assert_eq!(options.source, ProviderKind::PlanningSoftware);
        assert!(options.planning_server_requested);
        assert_eq!(options.startup(), Startup::Desk);

        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file("source lighting_desk\nhost old-desk\nport 5000\n", &options);
        assert_eq!(preferences.source, ProviderKind::PlanningSoftware);
        assert_eq!(preferences.host, "127.0.0.1");
        assert_eq!(preferences.port, 5311);
    }

    #[test]
    fn ports_outside_the_valid_range_are_rejected_with_a_readable_message() {
        assert!(parse_port("0").is_err());
        assert!(parse_port("65536").is_err());
        assert!(parse_port("nope").is_err());
        assert_eq!(parse_port(" 5000 "), Ok(5000));
    }

    #[test]
    fn staged_edits_validate_before_they_can_replace_the_live_connection() {
        let preferences = Preferences::from_options(&Options::default());
        let mut staged = StagedConnection::from_preferences(&preferences);
        staged.port_text = "70000".into();
        assert!(staged.validate().is_err());
        staged.port_text = "5001".into();
        staged.host = "  desk.local ".into();
        assert_eq!(staged.validate(), Ok(("desk.local".to_owned(), 5001)));
    }

    #[test]
    fn a_bare_launch_opens_the_planning_window() {
        // Nobody named a console and nobody named a file: there is nothing to draw and no way to
        // say what to draw, so the operator gets somewhere to choose.
        let options = Options::from_arguments(arguments(&[])).unwrap();
        assert_eq!(
            options.startup_with_recent_show(None, false),
            Startup::Planning
        );
    }

    #[test]
    fn a_named_desk_is_visualized_directly() {
        let options = Options::from_arguments(arguments(&["--server", "10.0.0.9"])).unwrap();
        assert_eq!(options.startup(), Startup::Desk);
        let options = Options::from_arguments(arguments(&["--port", "5001"])).unwrap();
        assert_eq!(
            options.startup(),
            Startup::Desk,
            "naming only the port still names a desk"
        );
    }

    #[test]
    fn a_show_file_is_served_and_visualized_without_a_planning_window() {
        let options = Options::from_arguments(arguments(&["--show", "/shows/tour.show"])).unwrap();
        assert_eq!(
            options.startup(),
            Startup::Show(PathBuf::from("/shows/tour.show"))
        );
    }

    #[test]
    fn a_named_desk_wins_over_an_absent_one() {
        // The desk launches the visualizer with its own address; that must never be mistaken for
        // a bare launch and turned into a planning window on an operator's show surface.
        let options =
            Options::from_arguments(arguments(&["--server", "127.0.0.1", "--port", "5000"]))
                .unwrap();
        assert_eq!(options.startup(), Startup::Desk);
    }

    #[test]
    fn the_demo_scene_overrides_every_other_source() {
        let options =
            Options::from_arguments(arguments(&["--demo", "--show", "/shows/tour.show"])).unwrap();
        assert_eq!(options.startup(), Startup::Demo);
    }

    #[test]
    fn quality_defaults_to_following_the_source() {
        let preferences = Preferences::from_options(&Options::default());
        assert_eq!(preferences.quality_label(), "Follow source");
    }
}

#[cfg(test)]
mod preference_tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    /// What the operator last left the window set to is what they find next time.
    #[test]
    fn preferences_survive_being_written_and_read_back() {
        let options = Options::default();
        let mut written = Preferences::from_options(&options);
        written.host = "10.0.0.9".into();
        written.port = 5310;
        written.quality_override = Some(RenderQuality::High);
        written.atmosphere.amount = 0.24;
        written.persistence.decay_seconds = 0.06;
        written.persistence.falloff = 3.5;
        written.ambient = 0.11;
        written.exposure = 1.75;
        written.laser_brightness = 2.4;
        written.fog_variation = viz_scene::FogVariation {
            lamp_cloudiness: 0.2,
            lamp_turbulence: 0.3,
            laser_cloudiness: 0.4,
            laser_turbulence: 0.5,
        };
        written.theme = Theme::DarkOnLight;
        written.background = Some([0.02, 0.04, 0.08]);
        written.show_labels = false;
        written.show_selection = false;
        written.floor_grid = Some(false);
        written.blender = "/opt/blender".into();
        written.set_input(3, Some(viz_dmx::Protocol::Sacn));

        let mut read = Preferences::from_options(&options);
        read.adopt_file(&written.to_file(), &options);

        assert_eq!(read.host, "10.0.0.9");
        assert_eq!(read.port, 5310);
        assert_eq!(read.quality_override, Some(RenderQuality::High));
        assert!((read.atmosphere.amount - 0.24).abs() < 1e-6);
        assert!((read.persistence.decay_seconds - 0.06).abs() < 1e-6);
        assert!((read.persistence.falloff - 3.5).abs() < 1e-6);
        assert!((read.ambient - 0.11).abs() < 1e-6);
        assert!((read.exposure - 1.75).abs() < 1e-6);
        assert!((read.laser_brightness - 2.4).abs() < 1e-6);
        assert_eq!(read.fog_variation, written.fog_variation);
        assert_eq!(read.theme, Theme::DarkOnLight);
        assert_eq!(read.background, Some([0.02, 0.04, 0.08]));
        assert!(!read.show_labels);
        assert!(!read.show_selection);
        assert_eq!(read.floor_grid, Some(false));
        assert_eq!(read.blender, "/opt/blender");
        assert_eq!(read.input_for(3), Some(viz_dmx::Protocol::Sacn));
    }

    /// A window that opens with nothing on it looks broken, so the one gesture that empties the
    /// screen is not a setting: a file written by an earlier version is read without complaint and
    /// the overlays still come back.
    #[test]
    fn a_hidden_overlay_is_not_kept_between_launches() {
        let options = Options::default();
        let mut written = Preferences::from_options(&options);
        written.overlays_hidden = true;
        let text = written.to_file();
        assert!(!text.contains("overlays_hidden"), "{text}");

        let mut read = Preferences::from_options(&options);
        read.adopt_file("overlays_hidden true\ntheme dark_on_light\n", &options);
        assert!(!read.overlays_hidden);
        assert_eq!(
            read.theme,
            Theme::DarkOnLight,
            "the rest of the file still reads"
        );
    }

    /// The command line is what the operator is asking for now.
    #[test]
    fn what_this_launch_named_wins_over_what_was_stored() {
        let options =
            Options::from_arguments(arguments(&["--server", "192.168.1.5", "--fog", "10"]))
                .unwrap();
        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file("host 10.0.0.9\nport 5310\nfog 0.9\nambient 0.5\n", &options);

        assert_eq!(
            preferences.host, "192.168.1.5",
            "--server was asked for now"
        );
        assert_eq!(preferences.port, 5000, "--server names this connection");
        assert!((preferences.atmosphere.amount - 0.1).abs() < 1e-6, "--fog");
        assert!(
            (preferences.ambient - 0.5).abs() < 1e-6,
            "nothing named an ambient level, so the stored one stands"
        );
    }

    /// A file this build cannot read is a convenience gone, never a window that will not open.
    #[test]
    fn an_unreadable_preference_line_is_skipped_rather_than_fatal() {
        let options = Options::default();
        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file(
            "# a comment\nnonsense\nport not-a-port\nquality nonsense\nambient 0.2\n",
            &options,
        );
        assert_eq!(preferences.port, DEFAULT_DESK_PORT);
        assert_eq!(preferences.quality_override, None);
        assert!((preferences.ambient - 0.2).abs() < 1e-6);
    }

    /// Settings of this window on this machine, never of the show or the repository.
    #[test]
    fn preferences_are_kept_beside_the_operators_application_data() {
        let options = Options {
            preferences_path: Some(PathBuf::from("/tmp/named.conf")),
            ..Options::default()
        };
        assert_eq!(
            preferences_path(&options),
            Some(PathBuf::from("/tmp/named.conf")),
            "a named path is used as given"
        );

        let default = preferences_path(&Options::default());
        if let Some(path) = default {
            let text = path.display().to_string();
            assert!(
                text.to_lowercase().contains("tosklight"),
                "kept with the application's own data: {text}"
            );
        }
    }
}

#[cfg(test)]
mod source_precedence_tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    /// A launch that named what to look at is not overruled by what was left selected last time.
    #[test]
    fn a_named_show_file_is_the_source_whatever_was_stored() {
        let options = Options::from_arguments(arguments(&["--show", "/shows/tour.show"])).unwrap();
        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file("source planning_software\n", &options);
        assert_eq!(preferences.source, ProviderKind::LightingDesk);
        assert_eq!(options.startup(), Startup::Show("/shows/tour.show".into()));
    }

    #[test]
    fn a_named_desk_is_the_source_whatever_was_stored() {
        let options = Options::from_arguments(arguments(&["--server", "10.0.0.9"])).unwrap();
        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file("source planning_software\n", &options);
        assert_eq!(preferences.source, ProviderKind::LightingDesk);
    }

    /// A launch that named nothing goes back to whatever the operator was last looking at.
    #[test]
    fn a_bare_launch_keeps_the_source_the_operator_left_selected() {
        let options = Options::from_arguments(arguments(&[])).unwrap();
        let mut preferences = Preferences::from_options(&options);
        preferences.adopt_file("source planning_software\n", &options);
        assert_eq!(preferences.source, ProviderKind::PlanningSoftware);
    }
}

#[cfg(test)]
mod launch_order {
    use super::*;

    fn bare() -> Options {
        Options::default()
    }

    /// The order TL-68 settles on: an explicit argument first, then the document the product had
    /// open last, and only then the editor. Everything above the recent document is asserted here;
    /// the recent lookup itself reads the operator's own configuration directory, so it is covered
    /// where that resolution lives rather than by a test that would depend on this machine.
    /// The desk owns a helper completely: nothing this process might otherwise have opened —
    /// a remembered document, the planning window — may take precedence over what it is told.
    #[test]
    fn nothing_overrides_being_a_helper() {
        let options = Options {
            helper: true,
            demo: true,
            desk_requested: true,
            show: Some(PathBuf::from("/shows/tour.show")),
            ..Options::default()
        };
        assert_eq!(options.startup(), Startup::Helper);
    }

    #[test]
    fn an_explicit_argument_beats_everything() {
        let mut options = bare();
        options.demo = true;
        assert_eq!(options.startup(), Startup::Demo);

        let mut options = bare();
        options.show = Some(PathBuf::from("/shows/tour.show"));
        options.desk_requested = true;
        assert_eq!(
            options.startup(),
            Startup::Show(PathBuf::from("/shows/tour.show")),
            "a named show wins over a named desk"
        );

        let mut options = bare();
        options.desk_requested = true;
        assert_eq!(
            options.startup(),
            Startup::Desk,
            "a named desk wins over the last document"
        );
    }
}
