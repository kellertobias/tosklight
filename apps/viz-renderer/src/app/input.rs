//! Turning what the operator does with the mouse and keyboard into camera movement.
//!
//! The gestures are the same in every view, and what differs is what each view lets them mean: a
//! plan pans and zooms and never turns, a perspective view turns and walks. Keeping that in one
//! place is what stops the two drifting apart.

use super::{Application, LOOK_RADIANS_PER_UNIT, pan_pixels, trace_input};
use crate::session::Session;
use crate::ui;
use viz_render::ResolvedCamera;
use viz_scene::ViewMode;
use winit::keyboard::{Key, ModifiersState};

impl Application {
    /// The number keys, in the order an operator reaches for them.
    pub(super) fn view_for_key(key: &str) -> Option<ViewMode> {
        match key {
            "1" => Some(ViewMode::Full3d),
            "2" => Some(ViewMode::TopDown),
            "3" => Some(ViewMode::FrontToBack),
            "4" => Some(ViewMode::LeftToRight),
            "5" => Some(ViewMode::RightToLeft),
            "6" => Some(ViewMode::Simple3d),
            "7" => Some(ViewMode::BackToFront),
            "8" => Some(ViewMode::Lines3d),
            _ => None,
        }
    }

    pub(super) fn quick_settings_chord(&self, key: &Key) -> bool {
        let comma = matches!(key.as_ref(), Key::Character(","));
        comma && (self.modifiers.super_key() || self.modifiers.control_key())
    }

    /// Whether a right-button drag moves the view across the floor instead of turning the camera.
    ///
    /// A plan view is defined by its axis and has no heading to turn, so the drag has to keep
    /// moving the picture there or the plan views lose their only mouse navigation. `Shift` asks
    /// for that same pan in a perspective view.
    pub(super) fn right_drag_pans(mode: ViewMode, modifiers: ModifiersState) -> bool {
        mode.is_orthographic() || modifiers.shift_key()
    }

    /// How many physical pixels the display this window is on puts in one point of hand movement.
    pub(super) fn window_scale(&self) -> f32 {
        self.window
            .as_ref()
            .map(|window| window.scale_factor())
            .unwrap_or(1.0) as f32
    }

    /// Turn the camera on the spot — pan and tilt — for one step of hand movement, in points.
    ///
    /// The camera follows the hand: it stays where it is and swings its aim, so movement to the
    /// right turns the camera right and movement downwards tilts it down. A plan view has no
    /// heading to turn, so there the same movement takes the picture across the floor instead;
    /// `Shift` asks for that in a perspective view too.
    ///
    /// This is what a right-button drag does, however the platform reports it. A mouse utility
    /// that has claimed the right button for its own panning delivers the drag as continuous
    /// scrolling, and the gesture has to mean the same thing when it arrives that way.
    pub(super) fn turn_camera(&mut self, hand_right: f32, hand_down: f32, scale: f32) {
        if hand_right == 0.0 && hand_down == 0.0 {
            return;
        }
        let mode = self.view_mode();
        if Self::right_drag_pans(mode, self.modifiers) {
            // The picture follows the hand: the stage moves with the drag.
            let (right, down) = (pan_pixels(hand_right, scale), pan_pixels(hand_down, scale));
            self.camera.pan_floor_plane(mode, -right, -down);
        } else {
            self.camera.look(
                mode,
                -hand_right * LOOK_RADIANS_PER_UNIT,
                hand_down * LOOK_RADIANS_PER_UNIT,
            );
        }
        self.latch_local_camera_control();
    }

    /// Move the camera for one step of a held drag, in points of hand movement.
    ///
    /// A pan is calibrated in physical pixels, so the picture keeps up with the pointer; a turn is
    /// calibrated on the hand, because how far the hand travelled is what should decide how far the
    /// camera swings, on any display.
    pub(super) fn drag_camera(&mut self, hand_right: f32, hand_down: f32, scale: f32) {
        if hand_right == 0.0 && hand_down == 0.0 {
            return;
        }
        let (right, down) = (pan_pixels(hand_right, scale), pan_pixels(hand_down, scale));
        trace_input(&format!("drag {right:.1},{down:.1}"));
        let mode = self.view_mode();
        if self.panning_camera_plane {
            self.camera.pan_camera_plane(mode, -right, down);
            self.latch_local_camera_control();
        }
        if self.turning {
            self.turn_camera(hand_right, hand_down, scale);
        }
        trace_input(&format!(
            "camera yaw {:.3} pitch {:.3} target {:.2},{:.2},{:.2}",
            self.camera.yaw,
            self.camera.pitch,
            self.camera.target.x,
            self.camera.target.y,
            self.camera.target.z
        ));
    }

    /// Whether the platform's command modifier is held, which turns a letter into a command.
    pub(super) fn command_chord(&self) -> bool {
        self.modifiers.super_key() || self.modifiers.control_key()
    }

    /// Track a held movement key. Returns `true` when the key was one.
    ///
    /// A letter held with the command key is a command and not a walk: `Cmd`+`S` keeps the picture
    /// rather than stepping the camera backwards. The key is still tracked so that letting go of a
    /// walk that a chord interrupted can never leave the camera drifting.
    pub(super) fn track_walk_key(&mut self, key: &Key, pressed: bool) -> bool {
        let Key::Character(character) = key.as_ref() else {
            return false;
        };
        let chord = self.command_chord();
        let target = match character.to_ascii_lowercase().as_str() {
            "w" => &mut self.walk_keys.forward,
            "s" => &mut self.walk_keys.back,
            "a" => &mut self.walk_keys.left,
            "d" => &mut self.walk_keys.right,
            _ => return false,
        };
        *target = pressed && !chord;
        true
    }

    /// Apply held movement keys for one frame.
    pub(super) fn apply_walk(&mut self, mode: ViewMode, delta_seconds: f32) {
        let forward = f32::from(self.walk_keys.forward) - f32::from(self.walk_keys.back);
        let right = f32::from(self.walk_keys.right) - f32::from(self.walk_keys.left);
        if forward == 0.0 && right == 0.0 {
            return;
        }
        // Walking speed scales with how far out the operator is zoomed, so it feels the same
        // whether they are inspecting one fixture or looking at the whole rig.
        let reach = if mode.is_orthographic() {
            self.camera.orthographic_size
        } else {
            self.camera.distance
        };
        let metres = (reach * 0.6).clamp(1.5, 40.0) * delta_seconds;
        self.camera
            .walk_floor_plane(mode, forward * metres, right * metres);
        self.latch_local_camera_control();
    }

    /// Inspect whatever the operator clicked on.
    ///
    /// Selecting a fixture answers the question an operator actually asks of a picture — *which
    /// light is that?* — so the bar names it, numbers it and gives its address. Clicking away from
    /// every fixture clears the selection rather than keeping a stale one.
    pub(super) fn select_under_cursor(&mut self) {
        let (width, height) = self.surface_size();
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if session.scene.fixtures.is_empty() {
            return;
        }
        let mut view = session.effective_view(&self.preferences);
        if self.camera_is_local {
            view.camera = self.camera.camera(&view.camera, view.mode);
        }
        let camera = ResolvedCamera::resolve(
            &view.camera,
            view.mode,
            width / height.max(1.0),
            session.scene.framing_bounds(),
        );
        let ray = camera.ray_through(self.cursor.0 as f32, self.cursor.1 as f32, width, height);
        let pick = viz_render::pick(
            &session.scene,
            &ray,
            self.camera.distance.max(1.0),
            &session.values.position_points,
        );
        self.selected = match pick.element {
            viz_render::PickedElement::Fixture(index) => session
                .scene
                .fixtures
                .get(index)
                .map(|fixture| fixture.instance_id),
            // Scenery and empty space are not fixtures, and neither is something to inspect yet.
            _ => None,
        };
    }

    /// What the status surface says about the selected fixture, if it is still in the scene.
    ///
    /// A selection is held by identity rather than by index, so a rig that is repatched underneath
    /// it either still has that fixture or no longer does — it never silently becomes another one.
    pub(super) fn selection_summary(
        session: &Session,
        selected: Option<uuid::Uuid>,
    ) -> Option<String> {
        let selected = selected?;
        let (index, fixture) = session
            .scene
            .fixtures
            .iter()
            .enumerate()
            .find(|(_, fixture)| fixture.instance_id == selected)?;
        let number = fixture
            .number
            .map(|number| format!("#{number} "))
            .unwrap_or_default();
        let address = match fixture.address {
            Some((universe, address)) => format!("U{universe}.{address}"),
            None => "unpatched".to_owned(),
        };
        // The brightest head of the fixture is what an operator means by "is it on?".
        let intensity = session
            .scene
            .emitters
            .iter()
            .enumerate()
            .filter(|(_, emitter)| emitter.fixture_index as usize == index)
            .filter_map(|(emitter, _)| session.values.emitters.get(emitter))
            .map(viz_scene::EmitterValues::visible_intensity)
            .fold(0.0_f32, f32::max);
        Some(format!(
            "{number}{}  {address}  {:.0}%",
            fixture.name,
            intensity * 100.0
        ))
    }

    /// The status-surface region under the cursor, if any.
    pub(super) fn hotspot_under_cursor(&self) -> Option<ui::Hotspot> {
        if self.preferences.overlays_hidden {
            return None;
        }
        ui::hotspot_at(&self.hotspots, self.cursor.0 as f32, self.cursor.1 as f32)
    }

    /// Turn a wheel notch over a value into a change to that value.
    pub(super) fn adjust_hotspot(&mut self, hotspot: ui::Hotspot, amount: f32) {
        match hotspot {
            ui::Hotspot::Fog => {
                self.preferences.atmosphere.amount =
                    (self.preferences.atmosphere.amount + amount * 0.02).clamp(0.0, 1.0);
            }
            ui::Hotspot::Exposure => {
                self.preferences.exposure =
                    (self.preferences.exposure * (1.0 + amount * 0.06)).clamp(0.05, 8.0);
            }
            ui::Hotspot::Ambient => {
                self.preferences.ambient =
                    (self.preferences.ambient + amount * 0.01).clamp(0.0, 1.0);
            }
            ui::Hotspot::OpenSettings => {}
        }
    }
}
