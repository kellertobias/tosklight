//! Freeze once at invocation; probing, rendering and writing belong to the isolated controller.
#[cfg(test)]
#[path = "still_tests.rs"]
mod tests;
#[path = "still_ui.rs"]
mod ui;
use super::Application;
use crate::stills::{Engine, Phase, StillOptions};
use std::sync::Arc;
pub(super) use ui::StillUi;
use viz_render::still::{FrozenStill, StillOrigin};
use viz_scene::ViewConfiguration;
use winit::{
    event::MouseScrollDelta,
    keyboard::{Key, ModifiersState, NamedKey},
};

pub(super) struct ResolvedFrame {
    pub view: ViewConfiguration,
    pub time: f32,
    pub scene_revision: u64,
    pub show_id: Option<uuid::Uuid>,
    pub values_frame: u64,
}

pub(super) fn shortcut(key: &Key, modifiers: ModifiersState) -> bool {
    let command = if cfg!(target_os = "macos") {
        modifiers.super_key() && !modifiers.control_key()
    } else {
        modifiers.control_key() && !modifiers.super_key()
    };
    command
        && modifiers.shift_key()
        && !modifiers.alt_key()
        && matches!(key.as_ref(), Key::Character("r") | Key::Character("R"))
}

pub(super) fn shortcut_allowed(
    key: &Key,
    modifiers: ModifiersState,
    repeat: bool,
    ime: bool,
    blocked: bool,
) -> bool {
    shortcut(key, modifiers) && !repeat && !ime && !blocked
}

fn invocation_view(
    resolved: &ViewConfiguration,
    camera: &viz_render::CameraControl,
    local: bool,
) -> ViewConfiguration {
    let mut view = *resolved;
    if local {
        view.camera = camera.camera(&view.camera, view.mode);
    }
    view
}

impl Application {
    /// Modal entry/exit clears input captured by the live picture, including button releases
    /// swallowed by the chooser. A completed choice must never resume an old held camera drag.
    pub(super) fn observe_still(&mut self) {
        self.apply_still_status(self.still_jobs.status());
    }

    pub(super) fn apply_still_status(&mut self, status: Option<crate::stills::Status>) {
        let was_modal = self.still_ui.modal();
        self.still_ui.observe(status);
        if was_modal || self.still_ui.modal() {
            self.turning = false;
            self.panning_camera_plane = false;
            self.drag_moved_by_device = false;
            self.walk_keys = super::WalkKeys::default();
        }
    }

    pub(super) fn render_4k_image(&mut self) {
        if self.still_jobs.is_busy()
            || self.still_ui.modal()
            || self.ime_composing
            || self.quick_settings.editing
        {
            return;
        }
        let frozen = (|| {
            let session = self
                .session
                .as_ref()
                .ok_or("No show is available for a 4K image yet")?;
            let resolved = self
                .last_still_frame
                .as_ref()
                .ok_or("Wait for the first resolved frame before rendering a 4K image")?;
            if resolved.scene_revision != session.scene.revision
                || resolved.show_id != session.scene.show_id
                || resolved.values_frame != session.values.frame
                || resolved.view.mode != session.source_view.mode
            {
                return Err(
                    "Wait for the newly selected show/view to resolve before rendering a 4K image"
                        .into(),
                );
            }
            let view = invocation_view(&resolved.view, &self.camera, self.camera_is_local);
            // Camera gestures since the last draw are authoritative, but VM/physics values are
            // the last final resolved values. Never advance their state merely to take a still.
            let renderer = self
                .renderer
                .as_ref()
                .ok_or("Renderer is not ready for a 4K image")?;
            FrozenStill::new_with_crowd_amount(
                &session.scene,
                &session.values,
                &view,
                resolved.time,
                renderer.frozen_media(),
                StillOrigin {
                    pane_id: "architect".into(),
                    follow_preload: false,
                },
                renderer.crowd_amount(),
            )
        })();
        let result = frozen.and_then(|frozen| {
            self.still_jobs.start(
                Arc::new(frozen),
                StillOptions {
                    configured_blender: self.preferences.blender_path(),
                    ..StillOptions::default()
                },
            )
        });
        match result {
            Ok(_) => {
                self.quick_settings.open = false;
                self.turning = false;
                self.panning_camera_plane = false;
                self.walk_keys = super::WalkKeys::default();
                self.still_ui.reset();
                self.still_ui.observe(self.still_jobs.status());
            }
            Err(error) => self.still_ui.fail(error),
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn handle_still_key(&mut self, key: &Key) -> bool {
        if self.command_chord() || self.modifiers.alt_key() || self.ime_composing {
            return self.still_ui.modal();
        }
        if self.still_ui.modal() {
            let action = match key.as_ref() {
                Key::Character("u" | "U") | Key::Named(NamedKey::Enter) => Some(ui::Action::Ultra),
                Key::Character("b" | "B") => Some(ui::Action::Ultimate),
                Key::Named(NamedKey::Escape) => Some(ui::Action::Cancel),
                Key::Named(NamedKey::ArrowDown) => {
                    self.still_ui.scroll(1);
                    None
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.still_ui.scroll(-1);
                    None
                }
                _ => None,
            };
            if let Some(action) = action {
                self.still_action(action);
            }
            return true;
        }
        if key == &Key::Named(NamedKey::Escape) && self.still_ui.visible() {
            self.still_action(if self.still_jobs.is_busy() {
                ui::Action::Cancel
            } else {
                ui::Action::Close
            });
            return true;
        }
        false
    }

    pub(super) fn click_still_panel(&mut self) -> bool {
        if let Some(action) = self
            .still_ui
            .action_at(self.cursor.0 as f32, self.cursor.1 as f32)
        {
            self.still_action(action);
            return true;
        }
        self.still_ui
            .contains(self.cursor.0 as f32, self.cursor.1 as f32)
    }
    pub(super) fn scroll_still_panel(&mut self, delta: MouseScrollDelta) {
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 24.0,
        };
        self.still_ui.scroll(if amount > 0.0 { -1 } else { 1 });
    }
    fn still_action(&mut self, action: ui::Action) {
        let status = self.still_jobs.status();
        let result = match action {
            ui::Action::Ultra | ui::Action::Ultimate => status
                .as_ref()
                .ok_or("No image job is awaiting a choice".into())
                .and_then(|s| {
                    self.still_jobs.choose(
                        &s.job_id,
                        if action == ui::Action::Ultra {
                            Engine::Ultra
                        } else {
                            Engine::Ultimate
                        },
                    )
                }),
            ui::Action::Cancel => status
                .as_ref()
                .ok_or("No image job is active".into())
                .and_then(|s| self.still_jobs.cancel(&s.job_id)),
            ui::Action::Close => {
                self.still_ui.close();
                Ok(())
            }
            ui::Action::Open | ui::Action::Reveal => {
                if let Some(crate::stills::Status {
                    phase: Phase::Saved { path },
                    ..
                }) = status
                {
                    self.still_ui.open_file(path, action == ui::Action::Reveal);
                }
                Ok(())
            }
        };
        if let Err(error) = result {
            self.still_ui.fail(error);
        }
        self.observe_still();
    }
}
