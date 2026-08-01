//! Opening a show file directly, without a desk.
//!
//! The visualizer reads a show through the desk API, and a show file on disk is not that API.
//! Rather than teach the visualizer a second way to read a show — which would be a second place
//! for persisted-show compatibility to drift — it starts a private headless server pointed at the
//! file and connects to that. The server is the same one the desk runs, so every migration,
//! fixture-library lookup, and patch rule behaves exactly as it does on the desk.
//!
//! The server is private to this visualizer: it binds loopback on a port nothing else is using,
//! keeps its data in a scratch directory, and is stopped when the show is closed or the
//! application exits.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// A headless server this application started for one show file.
#[derive(Debug)]
pub struct HostedShow {
    child: Child,
    port: u16,
    path: PathBuf,
}

impl HostedShow {
    /// Start a private server on `path`.
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!("{} is not a file", path.display()));
        }
        let binary = server_binary()?;
        let port = free_port()?;
        let data_dir = scratch_directory(path)?;
        let child = Command::new(&binary)
            .arg("--data-dir")
            .arg(&data_dir)
            .arg("--show")
            .arg(path)
            .arg("--bind")
            .arg(format!("127.0.0.1:{port}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", binary.display()))?;
        Ok(Self {
            child,
            port,
            path: path.to_path_buf(),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file name alone, for the operator surface.
    pub fn label(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.display().to_string())
    }

    /// Whether the server is still running. A server that exited is reported rather than left to
    /// look like a connection that is merely slow.
    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

impl Drop for HostedShow {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Names the server to start, for a development tree or an unusual installation. A build that
/// ships the two together never needs it.
pub const SERVER_PATH_ENV: &str = "TOSKLIGHT_VIZ_HEADLESS";

/// The headless server that shipped beside this binary.
fn server_binary() -> Result<PathBuf, String> {
    if let Some(named) = std::env::var_os(SERVER_PATH_ENV).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(named);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{SERVER_PATH_ENV} names {}, which is not a file",
            path.display()
        ));
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the visualizer binary: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "the visualizer binary has no directory".to_owned())?;
    let name = if cfg!(windows) {
        "light-headless.exe"
    } else {
        "light-headless"
    };
    let candidate = directory.join(name);
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(format!(
        "{name} is not beside the visualizer at {}; build it with `cargo build -p light-headless`",
        directory.display()
    ))
}

/// A loopback port nothing else is listening on.
///
/// The port is released again before the server is told to use it, which is a race no operator
/// will ever lose in practice and the only portable way to ask the system for a free one.
fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|error| format!("no free port for a private server: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("no free port for a private server: {error}"))
}

/// Where the private server keeps its data. Repository-owned scratch work belongs under the
/// artifacts tree, and one directory per show file keeps repeat openings warm.
fn scratch_directory(show: &Path) -> Result<PathBuf, String> {
    let base = std::env::var_os("LIGHT_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".artifacts/tmp"));
    let key: String = show
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let directory = base.join("viz-shows").join(key.trim_matches('-'));
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("could not prepare {}: {error}", directory.display()))?;
    Ok(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_something_that_is_not_a_file_is_reported_not_guessed() {
        let error = HostedShow::open(Path::new("/definitely/not/here.show"))
            .expect_err("a missing show cannot be opened");
        assert!(error.contains("not a file"), "{error}");
    }

    #[test]
    fn two_shows_never_share_one_scratch_directory() {
        let first = scratch_directory(Path::new("/shows/tour.show")).expect("first");
        let second = scratch_directory(Path::new("/shows/gala.show")).expect("second");
        assert_ne!(first, second);
    }
}
