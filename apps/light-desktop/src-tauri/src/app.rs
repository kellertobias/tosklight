pub(crate) fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(crate::visualizer::Visualizer::default())
        .manage(crate::stage_pane::StagePanes::default())
        .on_menu_event(crate::menu::handle_event)
        .invoke_handler(tauri::generate_handler![
            crate::windows::list_console_displays,
            crate::windows::open_console_screen,
            crate::windows::close_console_screen,
            crate::windows::hide_console_screen,
            crate::visualizer::open_visualizer,
            crate::visualizer::close_visualizer,
            crate::visualizer::visualizer_state,
            crate::visualizer::visualizer_renderer,
            crate::stage_pane::stage_pane_available,
            crate::stage_pane::open_stage_pane,
            crate::stage_pane::set_stage_pane,
            crate::stage_pane::close_stage_pane,
            crate::stage_pane::stage_pane_input,
            crate::stage_pane::set_stage_pane_picture,
            crate::stage_pane::set_stage_pane_selection,
            crate::stage_pane::stage_pane_status,
            crate::stage_pane::take_stage_pane_benchmark_samples,
            crate::stage_pane::take_stage_pane_picks,
            crate::stage_pane::stage_pane_camera,
            crate::stage_pane::place_stage_pane_camera,
            crate::lifecycle::exit_desktop_app,
            crate::lifecycle::cancel_quit,
            crate::lifecycle::frontend_ready,
            crate::packaged_benchmark::packaged_stage_benchmark_config,
            crate::packaged_benchmark::packaged_stage_benchmark_prepared,
            crate::packaged_benchmark::focus_packaged_stage_benchmark_window,
            crate::packaged_benchmark::append_packaged_stage_benchmark_sample
        ])
        .setup(|app| {
            crate::host_window::install(app)?;
            crate::lifecycle::setup(app);
            crate::menu::install(app)?;
            crate::server::setup(app)?;
            crate::stage_pane::drive(&app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build ToskLight control UI")
        .run(|handle, event| crate::lifecycle::handle_run_event(handle, &event));
}
