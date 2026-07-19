#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod osc;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(osc::ClientState::default())
        .invoke_handler(tauri::generate_handler![
            osc::connect_osc,
            osc::send_control
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run ToskLight Hardware Controls")
}
