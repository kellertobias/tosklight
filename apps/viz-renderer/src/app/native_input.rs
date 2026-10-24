//! Native key routing retains event metadata until repeat and ownership checks have run.
use super::{Application, WalkKeys, still};
use winit::{event::ElementState, event_loop::ActiveEventLoop};

impl Application {
    /// Route a complete native key event before converting it into typed UI commands. Repeats,
    /// IME/modal ownership and held movement keys must share this one ordered input path.
    pub(super) fn keyboard_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: winit::event::KeyEvent,
    ) {
        if still::shortcut(&event.logical_key, self.modifiers) {
            if event.state == ElementState::Pressed
                && still::shortcut_allowed(
                    &event.logical_key,
                    self.modifiers,
                    event.repeat,
                    self.ime_composing,
                    self.quick_settings.open || self.still_ui.modal(),
                )
            {
                self.render_4k_image();
            }
            return;
        }
        if self.still_ui.modal() {
            self.walk_keys = WalkKeys::default();
            if event.state == ElementState::Pressed && !event.repeat {
                self.handle_key(
                    event_loop,
                    event.logical_key.clone(),
                    event.text.map(|t| t.to_string()),
                );
            }
            return;
        }
        let pressed = event.state == ElementState::Pressed;
        // Movement keys are held, not typed, so they are tracked in both directions and
        // never reach the Quick Settings text fields.
        let movement = self.track_walk_key(&event.logical_key, pressed);
        if movement && !self.quick_settings.open && !self.command_chord() {
            return;
        }
        if pressed {
            self.handle_key(
                event_loop,
                event.logical_key.clone(),
                event.text.map(|text| text.to_string()),
            );
        }
    }
}
