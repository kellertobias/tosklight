use crate::server;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use tauri::{Emitter, Manager};

// Cmd+Q asks for confirmation once; the second press within the armed state actually quits.
#[derive(Default)]
pub(crate) struct QuitState {
    armed: AtomicBool,
}

impl QuitState {
    fn arm_or_confirm(&self) -> bool {
        self.armed.swap(true, Ordering::AcqRel)
    }

    fn cancel(&self) {
        self.armed.store(false, Ordering::Release);
    }
}

pub(crate) fn setup(app: &mut tauri::App) {
    app.manage(QuitState::default());
}

#[tauri::command]
pub(crate) fn exit_desktop_app(app: tauri::AppHandle) {
    let _ = app.emit("app-shutting-down", ());
    app.exit(0);
}

#[tauri::command]
pub(crate) fn cancel_quit(app: tauri::AppHandle, state: tauri::State<'_, QuitState>) {
    state.cancel();
    let _ = app.emit("quit-cancelled", ());
}

#[tauri::command]
pub(crate) fn frontend_ready(app: tauri::AppHandle) {
    if crate::packaged_benchmark::local_capture_requested() {
        static STARTED: AtomicBool = AtomicBool::new(false);
        if STARTED.swap(true, Ordering::AcqRel) {
            return;
        }
        tauri::async_runtime::spawn(async move {
            match crate::packaged_benchmark::show_local_capture(app.clone()).await {
                Ok(()) => mark_frontend_ready(app),
                Err(error) => eprintln!("Local capture remains hidden: {error}"),
            }
        });
        return;
    }
    mark_frontend_ready(app);
}

fn mark_frontend_ready(app: tauri::AppHandle) {
    if let Some(marker) = std::env::var_os("LIGHT_DESKTOP_TEST_READY_FILE") {
        let _ = std::fs::write(
            marker,
            format!("{{\"ready\":true,\"server\":\"{}\"}}", server::address()),
        );
        if let Some(delay) = std::env::var_os("LIGHT_DESKTOP_TEST_AUTO_EXIT") {
            let delay = delay.to_string_lossy().parse::<u64>().unwrap_or(150);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(delay));
                app.exit(0);
            });
        }
    }
}

pub(crate) fn request_quit(app: &tauri::AppHandle) {
    if app.state::<QuitState>().arm_or_confirm() {
        let _ = app.emit("app-shutting-down", ());
        app.exit(0);
    } else {
        let _ = app.emit("quit-requested", ());
    }
}

pub(crate) fn handle_run_event(handle: &tauri::AppHandle, event: &tauri::RunEvent) {
    if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
        let _ = handle.emit("app-shutting-down", ());
        server::terminate(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::QuitState;
    use std::sync::atomic::Ordering;

    #[test]
    fn quit_requires_one_armed_request_before_confirmation() {
        let state = QuitState::default();
        assert!(!state.arm_or_confirm());
        assert!(state.armed.load(Ordering::Acquire));
        assert!(state.arm_or_confirm());
        state.cancel();
        assert!(!state.armed.load(Ordering::Acquire));
    }
}
