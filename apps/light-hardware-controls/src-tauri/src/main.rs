#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native_simulator;
mod osc;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .manage(osc::ClientState::default())
        .manage(native_simulator::SimulatorState::default())
        .invoke_handler(tauri::generate_handler![
            osc::connect_osc,
            osc::disconnect_osc,
            osc::send_control,
            native_simulator::connect_native_simulator,
            native_simulator::send_native_simulator_control,
            native_simulator::disconnect_native_simulator
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
