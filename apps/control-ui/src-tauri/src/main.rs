#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs::OpenOptions, net::{SocketAddr, TcpStream}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, thread, time::{Duration, Instant}};
use tauri::Manager;

struct ServerProcess { child: Arc<Mutex<Option<Child>>>, stop: Arc<AtomicBool> }

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock() && let Some(child) = child.as_mut() { let _ = child.kill(); let _ = child.wait(); }
    }
}

fn server_is_running() -> bool {
    TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], 5000)), Duration::from_millis(120)).is_ok()
}

fn debug_data_dir(executable: &Path) -> Option<PathBuf> {
    executable.ancestors().find(|path| path.file_name().is_some_and(|name| name == "target")).and_then(Path::parent).map(|root| root.join("light-data"))
}

fn launch_server(app: &tauri::AppHandle) -> Result<Option<Child>, Box<dyn std::error::Error>> {
    if server_is_running() { return Ok(None); }
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or("application executable has no parent directory")?;
    let binary_name = if cfg!(windows) { "light-server.exe" } else { "light-server" };
    let bundled = directory.join(binary_name);
    let server = if bundled.is_file() { bundled } else if cfg!(debug_assertions) { debug_data_dir(&executable).and_then(|data| data.parent().map(|root| root.join("target/debug").join(binary_name))).unwrap_or(bundled) } else { bundled };
    if !server.is_file() { return Err(format!("bundled Light server is missing at {}", server.display()).into()); }
    let data_dir = if cfg!(debug_assertions) { debug_data_dir(&executable).unwrap_or(app.path().app_data_dir()?) } else { app.path().app_data_dir()? };
    std::fs::create_dir_all(&data_dir)?;
    let log_path = data_dir.join("light-server.log");
    let stdout = OpenOptions::new().create(true).truncate(true).write(true).open(&log_path)?;
    let stderr = stdout.try_clone()?;
    let mut child = Command::new(server).arg("--data-dir").arg(&data_dir).stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr)).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if server_is_running() { return Ok(Some(child)); }
        if let Some(status) = child.try_wait()? { return Err(format!("bundled Light server exited during startup with {status}; see {}", log_path.display()).into()); }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill(); let _ = child.wait();
    return Err(format!("timed out waiting for bundled Light server; see {}", log_path.display()).into());
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let child = launch_server(app.handle()).map_err(|error| { eprintln!("failed to start bundled Light server: {error}"); error })?;
            let process = ServerProcess { child: Arc::new(Mutex::new(child)), stop: Arc::new(AtomicBool::new(false)) };
            let watched_child = Arc::clone(&process.child); let stop = Arc::clone(&process.stop); let handle = app.handle().clone();
            thread::spawn(move || while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_secs(1));
                let needs_restart = if let Ok(mut child) = watched_child.lock() { match child.as_mut() { Some(child) => child.try_wait().ok().flatten().is_some(), None => !server_is_running() } } else { false };
                if needs_restart { match launch_server(&handle) { Ok(next) => { if let Ok(mut child) = watched_child.lock() { *child = next; } }, Err(error) => eprintln!("failed to restart bundled Light server: {error}") } }
            });
            app.manage(process);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run ToskLight control UI");
}
