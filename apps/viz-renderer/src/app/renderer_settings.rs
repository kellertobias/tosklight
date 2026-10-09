//! Settings another surface changed while this window is connected to it.
//!
//! The Architect edits the same renderer-local settings this window keeps, and a change made
//! there reaches a running Visualizer live rather than at its next launch.

use super::Application;
use crate::session::Session;
use std::time::Instant;

impl Application {
    pub(super) fn adopt_connected_renderer_settings(&mut self, session: &mut Session) {
        let Some(update) = session.take_renderer_settings() else {
            return;
        };
        let before = self.preferences.to_file();
        let interfaces = self.preferences.listen_interfaces.clone();
        // A UI Connect has the same endpoint authority as an explicit CLI connection.
        // Keep launch options unchanged, while still adopting ordinary rendering settings.
        let mut adoption_options = self.options.clone();
        if self.source_authority == super::SourceAuthority::External {
            adoption_options.desk_requested = true;
        }
        self.preferences
            .adopt_file(&update.settings.to_file(), &adoption_options);
        if self.preferences.to_file() == before {
            return;
        }
        // Receivers are bound when the connection is made, so another network needs a new one.
        if self.preferences.listen_interfaces != interfaces {
            self.reconnect();
        }
        self.next_preferences_save = Instant::now();
        if self.quick_settings.open {
            self.quick_settings.refresh(&self.preferences);
        }
    }
}
