#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod renderer;

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use renderer::Renderer;
use tauri::{Manager, RunEvent, WindowEvent};

struct RenderLoop {
    running: Arc<AtomicBool>,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = Arc::new(
                tauri::window::WindowBuilder::new(app, "main")
                    .title("wgpu Moving Lights — pan, tilt, color, haze and shadows")
                    .inner_size(1100.0, 720.0)
                    .min_inner_size(720.0, 480.0)
                    .resizable(true)
                    .build()?,
            );

            let renderer = pollster::block_on(Renderer::new(window.clone()))
                .map_err(|error| error.to_string())?;
            let running = Arc::new(AtomicBool::new(true));
            app.manage(RenderLoop {
                running: running.clone(),
            });

            thread::Builder::new()
                .name("wgpu-moving-lights-renderer".into())
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
        .expect("failed to build the wgpu moving-lights experiment")
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
