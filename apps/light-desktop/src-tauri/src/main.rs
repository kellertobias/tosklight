#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod hardware_controls;
mod host_window;
mod lifecycle;
mod menu;
mod packaged_benchmark;
mod server;
mod stage_compositor;
mod stage_pane;
mod visualizer;
mod windows;

fn main() {
    // The composition root delegates lifecycle::setup(app), menu::install(app),
    // server::setup(app), and lifecycle::handle_run_event to the app builder.
    app::run();
}
