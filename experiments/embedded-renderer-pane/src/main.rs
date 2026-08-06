#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Can a native renderer live inside a Tauri pane with web UI drawing over it?
//!
//! This is the experiment TL-68 leaves open. The plan is explicit that a native surface merely
//! floating above a WebView is not acceptable: menus, dialogs, selection overlays and status have
//! to be able to draw *above* the rendered image where the pane contract requires it. So this
//! builds the arrangement the other way round from the obvious one:
//!
//! 1. the window is a plain native `tauri::Window` with no webview of its own, and `wgpu`
//!    presents straight to it;
//! 2. a child webview is added *on top* of that window, sized to fill it, with a transparent
//!    background;
//! 3. the web side owns the layout. It tells the renderer which rectangle is the 3D pane, and the
//!    renderer scissors itself to exactly that rectangle and leaves the rest alone.
//!
//! Everything the web side draws is therefore above the rendered image by construction, and the
//! renderer cannot paint over the chrome even if it wanted to.
//!
//! The cost is input: a WKWebView on top swallows mouse events over the whole window, and CSS
//! `pointer-events: none` does not change AppKit hit-testing. So the web side captures pane input
//! itself and forwards it as commands. That is not a workaround to be embarrassed about — it is
//! the same direction the events would travel anyway once the renderer is a separate helper
//! process — but it is a real design consequence, and the readout in the window measures it.

mod renderer;
mod state;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use renderer::Renderer;
use state::{PaneRect, Report, Shared};
use tauri::{
    LogicalPosition, LogicalSize, Manager, RunEvent, WebviewUrl, WindowEvent,
    webview::WebviewBuilder,
};

struct RenderLoop {
    running: Arc<AtomicBool>,
}

/// The web side has laid out its pane and is telling the renderer where it ended up.
#[tauri::command]
fn set_pane(shared: tauri::State<'_, Arc<Shared>>, pane: PaneRect) {
    shared.set_pane(pane);
}

/// Pane input the web side captured and forwarded, because it cannot let it through.
#[tauri::command]
fn orbit(shared: tauri::State<'_, Arc<Shared>>, delta_x: f32, delta_y: f32) {
    shared.orbit(delta_x, delta_y);
}

#[tauri::command]
fn zoom(shared: tauri::State<'_, Arc<Shared>>, delta: f32) {
    shared.zoom(delta);
}

#[tauri::command]
fn report(shared: tauri::State<'_, Arc<Shared>>) -> Report {
    shared.report()
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Shared::default()))
        .invoke_handler(tauri::generate_handler![set_pane, orbit, zoom, report])
        .setup(|app| {
            // A window, not a webview window: nothing owns this surface but the renderer.
            let window = Arc::new(
                tauri::window::WindowBuilder::new(app, "main")
                    .title("Embedded Renderer Pane — native 3D under web chrome")
                    .inner_size(1180.0, 760.0)
                    .min_inner_size(820.0, 520.0)
                    .resizable(true)
                    .build()?,
            );

            let size = window.inner_size()?;
            let scale = window.scale_factor()?;
            let logical = size.to_logical::<f64>(scale);

            // The chrome, on top, transparent everywhere it does not paint. This is the claim
            // being tested: a menu opened here has to appear over the rendered image.
            window.add_child(
                WebviewBuilder::new("ui", WebviewUrl::App("index.html".into()))
                    .transparent(true)
                    .auto_resize(),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(logical.width, logical.height),
            )?;

            let shared = app.state::<Arc<Shared>>().inner().clone();
            let renderer = pollster::block_on(Renderer::new(window.clone(), shared.clone()))
                .map_err(|error| error.to_string())?;
            let running = Arc::new(AtomicBool::new(true));
            app.manage(RenderLoop {
                running: running.clone(),
            });

            thread::Builder::new()
                .name("embedded-renderer-pane".into())
                .spawn(move || {
                    let mut renderer = renderer;
                    while running.load(Ordering::Relaxed) {
                        renderer.render();
                        thread::sleep(Duration::from_millis(16));
                    }
                })
                .map_err(|error| error.to_string())?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the embedded renderer pane experiment")
        .run(|app, event| match event {
            RunEvent::Exit | RunEvent::ExitRequested { .. } => {
                app.state::<RenderLoop>()
                    .running
                    .store(false, Ordering::Relaxed);
            }
            RunEvent::WindowEvent {
                event: WindowEvent::Destroyed,
                ..
            } => {
                app.state::<RenderLoop>()
                    .running
                    .store(false, Ordering::Relaxed);
            }
            _ => {}
        });
}
