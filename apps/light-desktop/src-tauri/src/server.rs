use std::{
    fs::OpenOptions,
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::Manager;

pub(crate) struct ServerProcess {
    child: Arc<Mutex<Option<Child>>>,
    stop: Arc<AtomicBool>,
}

impl ServerProcess {
    fn terminate(&self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn parsed_server_address(value: Option<&str>) -> SocketAddr {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 5000)))
}

pub(crate) fn address() -> SocketAddr {
    parsed_server_address(std::env::var("LIGHT_DESKTOP_TEST_BIND").ok().as_deref())
}

fn is_running(address: SocketAddr) -> bool {
    TcpStream::connect_timeout(&address, Duration::from_millis(120)).is_ok()
}

fn debug_target_dir() -> PathBuf {
    option_env!("LIGHT_CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/build/cargo")
        })
}

fn debug_data_dir() -> PathBuf {
    option_env!("LIGHT_RUNTIME_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/runtime/light-data")
        })
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "light-headless.exe"
    } else {
        "light-headless"
    }
}

// @tour rust-by-example:40 Supervise the bundled server at the Tauri edge
// The desktop host reuses a local server or launches the packaged binary with explicit data,
// fixture-library, bind, logging, readiness, timeout, and child-exit handling.
fn launch(app: &tauri::AppHandle) -> Result<Option<Child>, Box<dyn std::error::Error>> {
    let address = address();
    if is_running(address) {
        return Ok(None);
    }
    let executable = std::env::current_exe()?;
    let directory = executable
        .parent()
        .ok_or("application executable has no parent directory")?;
    let bundled = directory.join(binary_name());
    let server = if bundled.is_file() {
        bundled
    } else if cfg!(debug_assertions) {
        debug_target_dir().join("debug").join(binary_name())
    } else {
        bundled
    };
    if !server.is_file() {
        return Err(format!("bundled Light server is missing at {}", server.display()).into());
    }
    let data_dir = std::env::var_os("LIGHT_DESKTOP_TEST_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or(if cfg!(debug_assertions) {
            std::env::var_os("LIGHT_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(debug_data_dir)
        } else {
            app.path().app_data_dir()?
        });
    let fixture_package_dir = app.path().resource_dir()?.join("fixture-library");
    std::fs::create_dir_all(&data_dir)?;
    let log_path = data_dir.join("light-headless.log");
    let stdout = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&log_path)?;
    let stderr = stdout.try_clone()?;
    let mut child = Command::new(server)
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("--fixture-package-dir")
        .arg(&fixture_package_dir)
        .arg("--bind")
        .arg(address.to_string())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()?;
    // Desk data with restored programmers and a compiled active show needs well over the
    // former 8s on a debug build; the child-exit check still fails fast on crashes.
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if is_running(address) {
            return Ok(Some(child));
        }
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "bundled Light server exited during startup with {status}; see {}",
                log_path.display()
            )
            .into());
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(format!(
        "timed out waiting for bundled Light server; see {}",
        log_path.display()
    )
    .into())
}

fn configure_frontend(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let url = format!("http://{}", address());
    let encoded = serde_json::to_string(&url)?;
    if std::env::var_os("LIGHT_DESKTOP_TEST_BIND").is_some() {
        window.eval(format!(
            "sessionStorage.setItem('light.test-server-url',{encoded});location.reload()"
        ))?;
    } else if cfg!(debug_assertions) {
        window.eval(format!(
            "if(localStorage.getItem('light.server-url')!=={encoded}){{localStorage.setItem('light.server-url',{encoded});location.reload()}}"
        ))?;
    }
    Ok(())
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let child = launch(app.handle()).map_err(|error| {
        eprintln!("failed to start bundled Light server: {error}");
        error
    })?;
    configure_frontend(app)?;
    let process = ServerProcess {
        child: Arc::new(Mutex::new(child)),
        stop: Arc::new(AtomicBool::new(false)),
    };
    let watched_child = Arc::clone(&process.child);
    let stop = Arc::clone(&process.stop);
    let handle = app.handle().clone();
    thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            thread::sleep(Duration::from_secs(1));
            let needs_restart = if let Ok(mut child) = watched_child.lock() {
                match child.as_mut() {
                    Some(child) => child.try_wait().ok().flatten().is_some(),
                    None => !is_running(address()),
                }
            } else {
                false
            };
            if needs_restart {
                match launch(&handle) {
                    Ok(next) => {
                        if let Ok(mut child) = watched_child.lock() {
                            *child = next;
                        }
                    }
                    Err(error) => eprintln!("failed to restart bundled Light server: {error}"),
                }
            }
        }
    });
    app.manage(process);
    Ok(())
}

pub(crate) fn terminate(handle: &tauri::AppHandle) {
    handle.state::<ServerProcess>().terminate();
}

#[cfg(test)]
mod tests {
    use super::{binary_name, parsed_server_address};
    use std::net::SocketAddr;

    #[test]
    fn server_bind_defaults_and_accepts_the_test_override() {
        assert_eq!(
            parsed_server_address(None),
            SocketAddr::from(([127, 0, 0, 1], 5000))
        );
        assert_eq!(
            parsed_server_address(Some("127.0.0.1:51234")),
            SocketAddr::from(([127, 0, 0, 1], 51234))
        );
        assert_eq!(
            parsed_server_address(Some("invalid")),
            SocketAddr::from(([127, 0, 0, 1], 5000))
        );
    }

    #[test]
    fn packaged_server_name_matches_the_target_platform() {
        assert_eq!(
            binary_name(),
            if cfg!(windows) {
                "light-headless.exe"
            } else {
                "light-headless"
            }
        );
    }
}
