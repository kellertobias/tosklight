#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hardware_controls;
mod lifecycle;
mod menu;
mod server;
mod windows;

fn main() {
    tauri::Builder::default()
        .on_menu_event(menu::handle_event)
        .invoke_handler(tauri::generate_handler![
            windows::list_console_displays,
            windows::open_console_screen,
            windows::close_console_screen,
            windows::hide_console_screen,
            windows::open_stage_view_window,
            lifecycle::exit_desktop_app,
            lifecycle::cancel_quit,
            lifecycle::frontend_ready
        ])
        .setup(|app| {
            lifecycle::setup(app);
            menu::install(app)?;
            server::setup(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build ToskLight control UI")
        .run(|handle, event| lifecycle::handle_run_event(handle, &event));
}
