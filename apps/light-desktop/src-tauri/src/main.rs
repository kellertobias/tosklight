#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    tauri::Builder::default()
        .manage(visualizer::Visualizer::default())
        .manage(stage_pane::StagePane::default())
        .on_menu_event(menu::handle_event)
        .invoke_handler(tauri::generate_handler![
            windows::list_console_displays,
            windows::open_console_screen,
            windows::close_console_screen,
            windows::hide_console_screen,
            windows::open_stage_view_window,
            visualizer::open_visualizer,
            visualizer::close_visualizer,
            visualizer::visualizer_state,
            visualizer::visualizer_renderer,
            visualizer::send_visualizer_scene,
            visualizer::send_visualizer_values,
            stage_pane::stage_pane_available,
            stage_pane::open_stage_pane,
            stage_pane::set_stage_pane,
            stage_pane::close_stage_pane,
            stage_pane::stage_pane_input,
            stage_pane::stage_pane_status,
            lifecycle::exit_desktop_app,
            lifecycle::cancel_quit,
            lifecycle::frontend_ready,
            packaged_benchmark::packaged_stage_benchmark_config,
            packaged_benchmark::packaged_stage_benchmark_prepared,
            packaged_benchmark::focus_packaged_stage_benchmark_window,
            packaged_benchmark::append_packaged_stage_benchmark_sample
        ])
        .setup(|app| {
            // Before anything that looks for the main window: it is built here now rather than by
            // the configuration, so a renderer has a surface to draw the Stage into.
            host_window::install(app)?;
            lifecycle::setup(app);
            menu::install(app)?;
            server::setup(app)?;
            // The pane draws on the desk's own thread whether or not the interface asks, so the
            // picture keeps up with the rig rather than with the web layer's frame requests.
            stage_pane::drive(&app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build ToskLight control UI")
        .run(|handle, event| lifecycle::handle_run_event(handle, &event));
}
