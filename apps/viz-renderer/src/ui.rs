//! Quick Settings and the connection/diagnostics surface.
//!
//! The whole surface is keyboard driven and is built as plain data, so its wording, validation,
//! and staged-connection behaviour are unit tested without a window.

use viz_render::Overlay;
use viz_scene::{Theme, UniverseGrade};

mod quick_settings;
mod status;

pub use quick_settings::{QuickSettings, QuickSettingsOutcome, Row, build_quick_settings};
pub use status::{DmxCameraControlStatus, StatusModel, build_fixture_labels, build_status};

/// Palette for one theme. The plot themes have to work on paper as well as on a screen.
#[derive(Clone, Copy)]
pub struct Palette {
    pub panel: [f32; 4],
    pub text: [f32; 4],
    pub dim: [f32; 4],
    pub accent: [f32; 4],
    pub warn: [f32; 4],
    pub bad: [f32; 4],
    pub good: [f32; 4],
    pub waiting: [f32; 4],
    pub focus: [f32; 4],
}

/// Convert a colour written the way it is meant to look into the linear value the overlay has to
/// hand the sRGB surface.
///
/// Every overlay colour is authored in sRGB, because that is the space an operator picks a green
/// in. Writing those numbers straight out lets the surface convert them a second time, which is
/// what turns a signal green into a washed-out mint.
fn srgb(colour: [f32; 4]) -> [f32; 4] {
    fn to_linear(channel: f32) -> f32 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }
    [
        to_linear(colour[0]),
        to_linear(colour[1]),
        to_linear(colour[2]),
        colour[3],
    ]
}

impl Palette {
    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::LightOnDark => Self {
                panel: srgb([0.03, 0.035, 0.045, 0.9]),
                text: srgb([0.93, 0.95, 0.98, 1.0]),
                dim: srgb([0.68, 0.72, 0.78, 1.0]),
                // The state colours are signals, not decoration: saturated enough to read as
                // green, orange, and red across a dark room at a glance.
                accent: srgb([0.2, 0.72, 1.0, 1.0]),
                warn: srgb([1.0, 0.58, 0.0, 1.0]),
                bad: srgb([0.98, 0.14, 0.16, 1.0]),
                good: srgb([0.05, 0.85, 0.33, 1.0]),
                waiting: srgb([0.45, 0.48, 0.54, 1.0]),
                focus: srgb([0.12, 0.16, 0.22, 1.0]),
            },
            Theme::DarkOnLight => Self {
                panel: srgb([0.87, 0.88, 0.9, 0.92]),
                text: srgb([0.08, 0.09, 0.11, 1.0]),
                dim: srgb([0.36, 0.38, 0.42, 1.0]),
                accent: srgb([0.0, 0.32, 0.8, 1.0]),
                warn: srgb([0.92, 0.47, 0.0, 1.0]),
                bad: srgb([0.85, 0.05, 0.05, 1.0]),
                good: srgb([0.0, 0.62, 0.2, 1.0]),
                waiting: srgb([0.5, 0.52, 0.56, 1.0]),
                focus: srgb([0.76, 0.79, 0.84, 1.0]),
            },
        }
    }

    fn grade(&self, grade: UniverseGrade) -> [f32; 4] {
        match grade {
            UniverseGrade::Healthy => self.good,
            UniverseGrade::Degraded => self.warn,
            UniverseGrade::Critical => self.bad,
            UniverseGrade::Waiting => self.waiting,
        }
    }
}

/// A region of the status surface the operator can act on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hotspot {
    OpenSettings,
    Fog,
    Exposure,
    Ambient,
}

/// One hotspot and the pixels it occupies.
#[derive(Clone, Copy, Debug)]
pub struct HotspotRect {
    pub hotspot: Hotspot,
    /// `x`, `y`, `width`, `height` in physical pixels.
    pub rect: [f32; 4],
}

impl HotspotRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect[0]
            && x <= self.rect[0] + self.rect[2]
            && y >= self.rect[1]
            && y <= self.rect[1] + self.rect[3]
    }
}

/// Find the hotspot under the cursor, if any.
pub fn hotspot_at(hotspots: &[HotspotRect], x: f32, y: f32) -> Option<Hotspot> {
    hotspots
        .iter()
        .find(|candidate| candidate.contains(x, y))
        .map(|candidate| candidate.hotspot)
}

/// What the window says when there is no picture to draw.
///
/// A visualizer with nothing loaded used to be an empty page — and, when the source was an editor
/// with no document open, an empty page with "server readiness failed" written across it in red.
/// Neither is true: nothing has failed, there is simply nothing to draw yet. This is what stands
/// in its place until there is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SplashState {
    /// The source answered and holds no show. The operator opens one; nothing is wrong.
    NoShow,
    /// On the way to a picture: connecting, authenticating, reading the scene.
    Loading(String),
    /// Something actually went wrong, and the window has nothing to show because of it.
    Failed(String),
}

impl SplashState {
    /// The line under the product name.
    fn detail(&self) -> &str {
        match self {
            Self::NoShow => "No Show Loaded",
            Self::Loading(what) => what,
            Self::Failed(reason) => reason,
        }
    }
}

/// Draw the splash: the application mark, the product, and one line about why the stage is empty.
///
/// Centred as a block rather than each line to its own centre, so the mark, the name and the
/// reason read as one thing at any window size.
pub fn build_splash(overlay: &mut Overlay, state: &SplashState, width: f32, height: f32) {
    let palette = Palette::of(Theme::LightOnDark);
    // The mark scales with the window but stays a mark: big enough to be the thing you see,
    // never so big that a wide window turns it into wallpaper.
    let icon = (height * 0.22).clamp(96.0, 260.0).min(width * 0.4);
    let title_scale = (icon / 44.0).clamp(2.0, 6.0);
    let detail_scale = (title_scale * 0.45).max(1.5);
    let title = "ToskLight 3D";
    let detail = state.detail();
    let gap = icon * 0.16;
    let title_height = Overlay::line_height(title_scale);
    let detail_height = Overlay::line_height(detail_scale);
    let block = icon + gap + title_height + gap * 0.6 + detail_height;
    let top = ((height - block) * 0.5).max(0.0);

    overlay.icon(
        (width - icon) * 0.5,
        top,
        icon,
        // The mark carries its own colours; a full-strength tint leaves them alone.
        [1.0, 1.0, 1.0, 1.0],
    );
    let title_y = top + icon + gap;
    overlay.text(
        (width - Overlay::measure(title, title_scale)) * 0.5,
        title_y,
        title_scale,
        palette.text,
        title,
    );
    let detail_ink = match state {
        SplashState::Failed(_) => palette.bad,
        _ => palette.dim,
    };
    overlay.text(
        (width - Overlay::measure(detail, detail_scale)) * 0.5,
        title_y + title_height + gap * 0.6,
        detail_scale,
        detail_ink,
        detail,
    );
}

/// The application mark: the ToskLight icon itself, drawn from the overlay atlas.
fn draw_application_mark(
    overlay: &mut Overlay,
    _palette: &Palette,
    x: f32,
    y: f32,
    size: f32,
) -> [f32; 4] {
    overlay.icon(x, y, size, [1.0, 1.0, 1.0, 1.0]);
    [x, y, size, size]
}

/// A lighter version of a badge colour, for its outline.
fn brighten(colour: [f32; 4], factor: f32) -> [f32; 4] {
    [
        (colour[0] * factor).min(1.0),
        (colour[1] * factor).min(1.0),
        (colour[2] * factor).min(1.0),
        colour[3],
    ]
}

/// Ink that stays legible on a coloured badge.
fn contrasting_ink(background: [f32; 4]) -> [f32; 4] {
    let luminance = 0.2126 * background[0] + 0.7152 * background[1] + 0.0722 * background[2];
    if luminance > 0.35 {
        srgb([0.03, 0.04, 0.05, 1.0])
    } else {
        srgb([0.97, 0.98, 0.99, 1.0])
    }
}

fn ui_scale(width: f32) -> f32 {
    if width >= 2400.0 {
        3.0
    } else if width >= 1200.0 {
        2.0
    } else {
        1.5
    }
}

#[cfg(test)]
mod tests {
    use super::quick_settings::*;
    use super::status::*;
    use super::*;
    use crate::settings::Options;
    use crate::settings::Preferences;
    use viz_scene::{ConnectionState, ProviderDiagnostics, ProviderKind, RenderQuality, ViewMode};
    use viz_scene::{UniverseGrade, UniverseHealth};

    /// The status model a test wants to talk about, with everything else held still.
    fn status_model<'a>(
        connection: &'a ConnectionState,
        diagnostics: &'a ProviderDiagnostics,
        fixtures: usize,
        waiting_for_dmx: bool,
        notice: Option<(String, bool)>,
    ) -> StatusModel<'a> {
        StatusModel {
            connection,
            diagnostics,
            universes: &[],
            view_mode: ViewMode::Full3d,
            quality: RenderQuality::High,
            quality_is_local: false,
            theme: Theme::LightOnDark,
            fixtures,
            emitters: fixtures,
            lights: 0,
            frames_per_second: 60.0,
            latency_p50_millis: 0.0,
            latency_p95_millis: 0.0,
            latency_max_millis: 0.0,
            dmx_hz: 0.0,
            fog_percent: 50.0,
            ambient_percent: 6.0,
            degraded: false,
            exposure: 1.0,
            renderer: "test".into(),
            gpu_millis: None,
            waiting_for_dmx,
            camera_control: DmxCameraControlStatus::None,
            selection: None,
            notice,
        }
    }

    fn crowded_status_with(
        width: f32,
        notice: Option<(String, bool)>,
    ) -> Vec<viz_render::OverlayQuad> {
        let universes: Vec<UniverseHealth> = (1..=16)
            .map(|universe| UniverseHealth {
                universe,
                rate_hz: 44.0,
                grade: UniverseGrade::Healthy,
                accepted: 100,
                broken: 0,
                stale: false,
                protocol: Some(viz_scene::SourceProtocol::ArtNet),
            })
            .collect();
        let connection = ConnectionState::Connected {
            endpoint: "http://127.0.0.1:5055".into(),
            revision: 1,
        };
        let diagnostics = ProviderDiagnostics::default();
        let model = StatusModel {
            connection: &connection,
            diagnostics: &diagnostics,
            universes: &universes,
            view_mode: ViewMode::TopDown,
            quality: RenderQuality::High,
            quality_is_local: false,
            theme: Theme::LightOnDark,
            fixtures: 301,
            emitters: 383,
            lights: 130,
            frames_per_second: 60.0,
            latency_p50_millis: 12.0,
            latency_p95_millis: 24.0,
            latency_max_millis: 43.0,
            dmx_hz: 44.0,
            fog_percent: 50.0,
            ambient_percent: 6.0,
            degraded: true,
            exposure: 1.0,
            renderer: "test".into(),
            gpu_millis: None,
            waiting_for_dmx: true,
            camera_control: DmxCameraControlStatus::None,
            selection: None,
            notice,
        };
        let mut overlay = Overlay::default();
        build_status(&mut overlay, &model, width, 900.0);
        overlay.quads
    }

    #[test]
    fn pinning_one_universe_leaves_every_other_one_following_the_show() {
        let (mut settings, mut preferences) = fixture();
        settings.selected = select(&settings, Row::InputUniverse);
        settings.adjust(2, &mut preferences); // universe 1 -> 3
        assert_eq!(settings.input_universe, 3);

        settings.selected = select(&settings, Row::InputProtocol);
        settings.adjust(1, &mut preferences);
        assert_eq!(
            preferences.input_for(3),
            Some(viz_dmx::Protocol::ArtNet),
            "the operator pinned universe 3"
        );
        assert_eq!(preferences.input_for(1), None, "universe 1 was left alone");
        assert_eq!(preferences.input_for(2), None, "universe 2 was left alone");

        // Cycling back to the start clears the pin rather than leaving a stale one.
        settings.adjust(-1, &mut preferences);
        assert_eq!(preferences.input_for(3), None);
        assert!(preferences.input_overrides.is_empty());
    }

    /// A window with nothing in it has to say why, or it is indistinguishable from a broken one.
    #[test]
    fn an_empty_rig_says_so_rather_than_showing_an_empty_window() {
        let connected = ConnectionState::Connected {
            endpoint: "http://127.0.0.1:5310".into(),
            revision: 1,
        };
        let diagnostics = ProviderDiagnostics::default();
        let empty = status_model(&connected, &diagnostics, 0, false, None);
        assert!(matches!(
            second_row_note(&empty),
            Some(SecondRowNote::EmptyRig)
        ));
        assert!(
            second_row_note(&empty)
                .unwrap()
                .text()
                .contains("Viz editor")
        );

        // A rig that is there but dark is a different sentence.
        let dark = status_model(&connected, &diagnostics, 301, true, None);
        assert!(matches!(
            second_row_note(&dark),
            Some(SecondRowNote::WaitingForDmx)
        ));

        // What the operator just did comes first either way.
        let snapshot = status_model(
            &connected,
            &diagnostics,
            0,
            false,
            Some(("Snapshot saved".into(), false)),
        );
        assert!(matches!(
            second_row_note(&snapshot),
            Some(SecondRowNote::Notice("Snapshot saved", false))
        ));

        // Not connected yet: the connection line beside it already says so.
        let connecting = ConnectionState::Resolving {
            endpoint: "http://127.0.0.1:5310".into(),
        };
        let connecting = status_model(&connecting, &diagnostics, 0, false, None);
        assert!(second_row_note(&connecting).is_none());
    }

    #[test]
    fn local_camera_control_names_the_release_action_and_stale_hold() {
        let connected = ConnectionState::Connected {
            endpoint: "http://127.0.0.1:5310".into(),
            revision: 1,
        };
        let diagnostics = ProviderDiagnostics::default();
        let mut model = status_model(&connected, &diagnostics, 12, false, None);
        model.camera_control = DmxCameraControlStatus::Local { can_release: true };
        let note = second_row_note(&model).expect("local ownership is visible");
        assert_eq!(
            note.text(),
            "Local camera control \u{2014} press C to return to DMX"
        );

        model.camera_control = DmxCameraControlStatus::Local { can_release: false };
        assert!(
            second_row_note(&model)
                .expect("stale local ownership is visible")
                .text()
                .contains("waiting for the DMX camera")
        );

        model.camera_control = DmxCameraControlStatus::Held;
        assert_eq!(
            second_row_note(&model)
                .expect("held pose is visible")
                .text(),
            "DMX camera unavailable \u{2014} holding its last pose"
        );
    }

    #[test]
    fn diagnostics_reserve_gpu_time_width_and_put_the_show_on_its_own_row() {
        let connected = ConnectionState::Connected {
            endpoint: "http://127.0.0.1:5310".into(),
            revision: 1,
        };
        let mut diagnostics = ProviderDiagnostics {
            show_identity: "A deliberately long active show name".into(),
            scene_revision: 42,
            ..ProviderDiagnostics::default()
        };
        let mut model = status_model(&connected, &diagnostics, 12, false, None);
        model.renderer = "Apple M4 Max (Metal, 4× MSAA)".into();
        model.gpu_millis = Some(9.25);

        let one_digit = model.renderer_summary();
        assert!(one_digit.contains("GPU  9.25 ms"));
        let rows = diagnostic_rows(&model);
        assert_eq!(rows[1], "show A deliberately long active show name");
        assert!(!rows[0].contains("active show"));
        assert!(rows[2].contains("Apple M4 Max (Metal, 4× MSAA)"));

        model.gpu_millis = Some(10.25);
        let two_digits = model.renderer_summary();
        assert!(two_digits.contains("GPU 10.25 ms"));
        assert_eq!(one_digit.len(), two_digits.len());

        diagnostics.show_identity.clear();
        let empty_show = status_model(&connected, &diagnostics, 12, false, None);
        assert_eq!(diagnostic_rows(&empty_show)[1], "show -");
    }

    #[test]
    fn a_crowded_status_bar_never_overlaps_its_own_halves() {
        for width in [900.0_f32, 1280.0, 1600.0, 2560.0] {
            for notice in [
                None,
                Some((
                    "Snapshot 14:22:08 saved \u{2014} 301 fixtures, 130 live".into(),
                    false,
                )),
            ] {
                let quads = crowded_status_with(width, notice);
                assert!(!quads.is_empty(), "nothing drawn at {width}");
                for quad in &quads {
                    let right = quad.rect[0] + quad.rect[2];
                    assert!(
                        quad.rect[0] >= -0.5 && right <= width + 0.5,
                        "a status item left the bar at {width}: {:?}",
                        quad.rect
                    );
                }
            }
        }
    }

    #[test]
    fn the_snapshot_shortcut_is_the_first_hint_and_the_last_to_be_dropped() {
        let hints = shortcut_hints();
        assert!(
            hints[0].contains("snapshot"),
            "the snapshot shortcut leads: {hints:?}"
        );
        assert!(
            hints[0].contains("Cmd+S") || hints[0].contains("Ctrl+S"),
            "the hint has to name the key: {hints:?}"
        );

        // Shrink the space a pixel at a time; whatever survives always includes the snapshot.
        let scale = 2.0;
        let full = Overlay::measure(&hints.join(HINT_SEPARATOR), scale);
        let mut widths = Vec::new();
        for available in 0..=(full.ceil() as usize) {
            let text = fitting_hints(available as f32, scale);
            widths.push(text.len());
            if !text.is_empty() {
                assert!(
                    text.starts_with(hints[0]),
                    "the snapshot shortcut was dropped at {available}px: {text:?}"
                );
            }
        }
        assert!(
            widths.contains(&0),
            "nothing should be drawn when not even one hint fits"
        );
        assert_eq!(
            fitting_hints(full, scale),
            hints.join(HINT_SEPARATOR),
            "every hint should be shown when they all fit"
        );
    }

    #[test]
    fn the_shortcuts_are_drawn_on_a_bar_with_room_for_them() {
        // The bar is built as quads, so the check is that the hints claimed pixels on the second
        // row rather than being silently squeezed out of a window this wide.
        let bare = {
            let connection = ConnectionState::Idle;
            let diagnostics = ProviderDiagnostics::default();
            let model = StatusModel {
                connection: &connection,
                diagnostics: &diagnostics,
                universes: &[],
                view_mode: ViewMode::Full3d,
                quality: RenderQuality::High,
                quality_is_local: false,
                theme: Theme::LightOnDark,
                fixtures: 12,
                emitters: 12,
                lights: 4,
                frames_per_second: 60.0,
                latency_p50_millis: 1.0,
                latency_p95_millis: 2.0,
                latency_max_millis: 3.0,
                dmx_hz: 44.0,
                fog_percent: 50.0,
                ambient_percent: 6.0,
                degraded: false,
                exposure: 1.0,
                renderer: "test".into(),
                gpu_millis: None,
                waiting_for_dmx: false,
                camera_control: DmxCameraControlStatus::None,
                selection: None,
                notice: None,
            };
            let mut overlay = Overlay::default();
            build_status(&mut overlay, &model, 1920.0, 1080.0);
            overlay.quads.len()
        };
        let scale = (ui_scale(1920.0) * 0.62).max(1.0);
        let hint_glyphs = shortcut_hints()
            .join(HINT_SEPARATOR)
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();
        assert!(
            bare > hint_glyphs,
            "a 1920-wide bar should have room for every shortcut"
        );
        assert!(!fitting_hints(600.0, scale).is_empty());
    }

    /// Where a named row currently sits, which moves as snapshots are taken.
    fn select(settings: &QuickSettings, wanted: Row) -> usize {
        settings
            .rows()
            .iter()
            .position(|row| *row == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is not in the panel"))
    }

    fn fixture() -> (QuickSettings, Preferences) {
        let preferences = Preferences::from_options(&Options::default());
        let settings = QuickSettings::new(&preferences, false);
        (settings, preferences)
    }

    #[test]
    fn cancelling_restores_the_live_endpoint_and_closes() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.selected = 1;
        settings.editing = true;
        settings.type_character('9');
        assert_eq!(settings.staged.host, "127.0.0.19");
        settings.editing = false;
        settings.selected = select(&settings, Row::Cancel);
        assert_eq!(
            settings.activate(&mut preferences),
            QuickSettingsOutcome::Close
        );
        assert_eq!(preferences.host, "127.0.0.1");
        assert!(!settings.open);
    }

    #[test]
    fn connecting_applies_a_validated_endpoint() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.staged.host = "10.0.0.4".into();
        settings.staged.port_text = "5100".into();
        settings.selected = select(&settings, Row::Connect);
        assert_eq!(
            settings.activate(&mut preferences),
            QuickSettingsOutcome::Connect {
                host: "10.0.0.4".into(),
                port: 5100
            }
        );
        assert_eq!(preferences.port, 5100);
    }

    #[test]
    fn an_invalid_port_reports_in_place_without_changing_the_connection() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.staged.port_text = "0".into();
        settings.selected = select(&settings, Row::Connect);
        let outcome = settings.activate(&mut preferences);
        assert!(matches!(outcome, QuickSettingsOutcome::Invalid(_)));
        assert_eq!(preferences.port, 5000);
    }

    #[test]
    fn an_unavailable_planning_provider_is_named_rather_than_selectable() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.selected = 0;
        let outcome = settings.adjust(1, &mut preferences);
        assert!(matches!(outcome, QuickSettingsOutcome::Invalid(_)));
        assert_eq!(settings.staged.source, ProviderKind::LightingDesk);
        assert!(settings.message.contains("Not available in this build"));
    }

    #[test]
    fn quality_and_fog_apply_immediately_without_reconnecting() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.selected = select(&settings, Row::Quality);
        assert_eq!(
            settings.adjust(1, &mut preferences),
            QuickSettingsOutcome::AppliedLocally
        );
        assert_eq!(preferences.quality_override, Some(RenderQuality::Draft));
        settings.selected = select(&settings, Row::FogAmount);
        let before = preferences.atmosphere.amount;
        settings.adjust(1, &mut preferences);
        assert!(preferences.atmosphere.amount > before);
    }

    /// Laser brightness is the operator's own, applies without reconnecting, and goes all the way
    /// down to lasers off and well past the strength they are drawn at by default.
    #[test]
    fn laser_brightness_is_adjustable_from_off_to_stronger_than_default() {
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.selected = select(&settings, Row::LaserBrightness);
        assert!((preferences.laser_brightness - 1.0).abs() < 1e-6);

        assert_eq!(
            settings.adjust(1, &mut preferences),
            QuickSettingsOutcome::AppliedLocally
        );
        assert!(preferences.laser_brightness > 1.0);

        for _ in 0..64 {
            settings.adjust(-1, &mut preferences);
        }
        assert_eq!(preferences.laser_brightness, 0.0);
        assert!(settings.message.contains("Lasers off"));
        assert_eq!(settings.value(Row::LaserBrightness, &preferences), "0%");

        for _ in 0..128 {
            settings.adjust(1, &mut preferences);
        }
        assert_eq!(
            preferences.laser_brightness,
            crate::settings::MAX_LASER_BRIGHTNESS
        );
    }

    fn kept(count: usize) -> Vec<crate::snapshots::SnapshotRow> {
        (0..count)
            .map(|index| crate::snapshots::SnapshotRow {
                entry: viz_snapshot::SnapshotEntry {
                    directory: std::path::PathBuf::from(format!("/tmp/snap-{index}")),
                    captured_at: format!("2026-07-31 14:2{index}:00"),
                    show: "Tour".into(),
                    counts: viz_snapshot::SnapshotCounts {
                        fixtures: 301,
                        heads: 383,
                        live_beams: 130,
                        triangles: 90_000,
                    },
                    blend: None,
                },
                export: crate::snapshots::ExportState::Idle,
            })
            .collect()
    }

    #[test]
    fn the_focused_row_is_always_one_of_the_rows_that_is_drawn() {
        for rows in [1_usize, 5, 14, 26, 40] {
            for selected in 0..rows {
                for room in [1_usize, 4, 9, 26, 100] {
                    let (first, visible) = visible_window(rows, selected, room);
                    assert!(first + visible <= rows, "{rows}/{selected}/{room}");
                    assert!(visible > 0, "{rows}/{selected}/{room}");
                    assert!(
                        (first..first + visible).contains(&selected),
                        "selection {selected} fell outside {first}..{} of {rows} with room {room}",
                        first + visible
                    );
                }
            }
        }
    }

    #[test]
    fn a_short_list_is_drawn_whole_and_never_scrolled() {
        assert_eq!(visible_window(6, 0, 20), (0, 6));
        assert_eq!(visible_window(6, 5, 20), (0, 6));
    }

    #[test]
    fn a_panel_full_of_snapshots_stays_on_the_screen() {
        // The list grows with the operator's own work, and the panel is sized from it. A full
        // list must not run off the bottom of a small window or out of its own frame.
        let connection = ConnectionState::Idle;
        let diagnostics = ProviderDiagnostics::default();
        let model = StatusModel {
            connection: &connection,
            diagnostics: &diagnostics,
            universes: &[],
            view_mode: ViewMode::Full3d,
            quality: RenderQuality::High,
            quality_is_local: false,
            theme: Theme::LightOnDark,
            fixtures: 301,
            emitters: 383,
            lights: 130,
            frames_per_second: 60.0,
            latency_p50_millis: 1.0,
            latency_p95_millis: 2.0,
            latency_max_millis: 3.0,
            dmx_hz: 44.0,
            fog_percent: 50.0,
            ambient_percent: 6.0,
            degraded: false,
            exposure: 1.0,
            renderer: "test".into(),
            gpu_millis: None,
            waiting_for_dmx: false,
            camera_control: DmxCameraControlStatus::None,
            selection: None,
            notice: None,
        };
        for (width, height) in [(1280.0_f32, 720.0_f32), (2560.0, 1440.0), (1000.0, 640.0)] {
            let (mut settings, preferences) = fixture();
            settings.open = true;
            settings.snapshots = kept(viz_snapshot::DEFAULT_KEPT);
            settings.snapshot_folder = "/Users/operator/Library/Application Support/ToskLight/\
                                        Visualizer/Snapshots"
                .into();
            settings.selected = settings.rows().len() - 1;
            let mut overlay = Overlay::default();
            build_quick_settings(&mut overlay, &settings, &preferences, &model, width, height);
            assert!(!overlay.quads.is_empty(), "nothing drawn at {width}");
            for quad in &overlay.quads {
                assert!(
                    quad.rect[0] >= -0.5 && quad.rect[0] + quad.rect[2] <= width + 0.5,
                    "a row left the window at {width}x{height}: {:?}",
                    quad.rect
                );
                assert!(
                    quad.rect[1] >= -0.5 && quad.rect[1] + quad.rect[3] <= height + 0.5,
                    "the panel ran off the bottom at {width}x{height}: {:?}",
                    quad.rect
                );
            }
        }
    }

    #[test]
    fn every_kept_snapshot_gets_a_row_that_says_when_it_was_taken() {
        let (mut settings, preferences) = fixture();
        settings.toggle(&preferences);
        let without = settings.rows().len();
        settings.snapshots = kept(3);
        assert_eq!(settings.rows().len(), without + 3);
        assert_eq!(settings.row_label(Row::Snapshot(0)), "Snapshot 14:20:00");
        assert_eq!(
            settings.value(Row::Snapshot(1), &preferences),
            "301 fixtures, 130 live \u{2014} Enter exports"
        );
    }

    #[test]
    fn a_grown_list_still_reaches_the_rows_underneath_it() {
        // Connect and Cancel stay at the bottom however many captures are above them, and moving
        // up from the top row still wraps onto the last one rather than off the end.
        let (mut settings, _) = fixture();
        settings.snapshots = kept(4);
        let rows = settings.rows();
        assert_eq!(rows.last(), Some(&Row::Cancel));
        settings.selected = 0;
        settings.move_selection(-1);
        assert_eq!(settings.row(), Row::Cancel);
        settings.move_selection(1);
        assert_eq!(settings.row(), Row::Source);
    }

    #[test]
    fn activating_a_snapshot_asks_for_that_one_to_be_exported() {
        let (mut settings, mut preferences) = fixture();
        settings.snapshots = kept(3);
        settings.selected = select(&settings, Row::Snapshot(2));
        assert_eq!(
            settings.activate(&mut preferences),
            QuickSettingsOutcome::ExportSnapshot(2)
        );
        assert!(
            settings.message.contains("14:22:00"),
            "the operator should be told which one: {}",
            settings.message
        );
    }

    #[test]
    fn a_snapshot_already_being_exported_is_not_started_a_second_time() {
        let (mut settings, mut preferences) = fixture();
        settings.snapshots = kept(1);
        settings.snapshots[0].export = crate::snapshots::ExportState::Running;
        settings.selected = select(&settings, Row::Snapshot(0));
        assert_eq!(
            settings.activate(&mut preferences),
            QuickSettingsOutcome::None
        );
        assert!(settings.message.contains("already being exported"));
    }

    #[test]
    fn naming_a_blender_keeps_it_without_touching_the_connection() {
        // Where Blender lives has nothing to do with the desk, so it must not need Connect —
        // which would reconnect the session as a side effect of setting an export tool.
        let (mut settings, mut preferences) = fixture();
        settings.toggle(&preferences);
        settings.selected = select(&settings, Row::BlenderPath);
        settings.activate(&mut preferences);
        assert!(settings.editing);
        for character in "/opt/blender".chars() {
            settings.type_character(character);
        }
        settings.activate(&mut preferences);
        assert!(!settings.editing);
        assert_eq!(preferences.blender, "/opt/blender");
        assert_eq!(preferences.host, "127.0.0.1", "the endpoint was untouched");
    }

    #[test]
    fn the_blender_row_says_whether_one_was_found_at_all() {
        let (mut settings, preferences) = fixture();
        settings.toggle(&preferences);
        let value = settings.value(Row::BlenderPath, &preferences);
        assert!(
            value.starts_with("Found: ") || value.starts_with("Not found"),
            "an operator has to be able to tell the two apart: {value}"
        );
    }

    #[test]
    fn quality_cycles_back_to_following_the_source() {
        let mut current = None;
        for _ in 0..RenderQuality::ALL.len() + 1 {
            current = cycle_quality(current, 1);
        }
        assert_eq!(current, None);
    }
}
