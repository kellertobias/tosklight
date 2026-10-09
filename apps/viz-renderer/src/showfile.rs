//! Opening a show file directly, without a desk.
//!
//! The visualizer reads a show through the desk API, and a show file on disk is not that API.
//! Rather than teach the visualizer a second way to read a show — which would be a second place
//! for persisted-show compatibility to drift — it starts a private headless server pointed at a consistent private
//! snapshot of the file and connects to that. The server is the same one the desk runs, so every migration,
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
    _workspace: PreviewWorkspace,
}

impl HostedShow {
    /// Start a private server on a consistent snapshot of `path`.
    pub fn open(path: &Path) -> Result<Self, String> {
        if !path.is_file() {
            return Err(format!("{} is not a file", path.display()));
        }
        let binary = server_binary()?;
        let port = free_port()?;
        let workspace = PreviewWorkspace::create(&scratch_directory(path)?, path)?;
        let child = workspace.start(&binary, port)?;
        Ok(Self {
            child,
            port,
            path: path.to_path_buf(),
            _workspace: workspace,
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

/// Build the private server command without inheriting operator-facing control ports.
///
/// A show preview can run beside the real desk, which normally owns OSC port 9000. Binding the
/// private server to an ephemeral OSC port preserves its complete runtime while preventing a
/// preview-only helper from colliding with Control and exiting during startup.
fn private_server_command(binary: &Path, data_dir: &Path, show: &Path, port: u16) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--show")
        .arg(show)
        .arg("--bind")
        .arg(format!("127.0.0.1:{port}"))
        .arg("--osc-bind")
        .arg("127.0.0.1:0");
    command
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
/// artifacts tree; each opening creates its own owned workspace below this grouping directory.
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

/// Each opening owns a distinct private document and desk directory. The private server may
/// adopt or migrate that document; neither those writes nor a second opening touch the original.
#[derive(Debug)]
struct PreviewWorkspace {
    directory: PathBuf,
    snapshot: PathBuf,
}

impl PreviewWorkspace {
    fn create(parent: &Path, source: &Path) -> Result<Self, String> {
        let directory = parent.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&directory)
            .map_err(|error| format!("could not prepare {}: {error}", directory.display()))?;
        let mut workspace = Self {
            directory,
            snapshot: PathBuf::new(),
        };
        let filename = source
            .file_name()
            .ok_or_else(|| "show has no file name".to_owned())?;
        workspace.snapshot = snapshot_show(source, &workspace.directory.join(filename))?;
        Ok(workspace)
    }

    fn start(&self, binary: &Path, port: u16) -> Result<Child, String> {
        private_server_command(binary, &self.directory, &self.snapshot, port)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", binary.display()))
    }
}

impl Drop for PreviewWorkspace {
    fn drop(&mut self) {
        // Never remove the parent, original document or another opening's private files.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// SQLite's backup reads a consistent committed snapshot, including uncheckpointed WAL pages.
/// Opening read-only and never checkpointing is essential: ShowStore::backup_to checkpoints its
/// source and would therefore modify the operator's original file even before private adoption.
fn snapshot_show(source: &Path, destination: &Path) -> Result<PathBuf, String> {
    let connection =
        rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| format!("could not read show {}: {error}", source.display()))?;
    connection
        .backup(rusqlite::MAIN_DB, destination, None)
        .map_err(|error| format!("could not snapshot show {}: {error}", source.display()))?;
    Ok(destination.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

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

    #[test]
    fn private_show_server_does_not_claim_the_desk_osc_port() {
        let command = private_server_command(
            Path::new("light-headless"),
            Path::new("scratch"),
            Path::new("venue.show"),
            5311,
        );
        let arguments = command.get_args().collect::<Vec<_>>();
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == [OsStr::new("--osc-bind"), OsStr::new("127.0.0.1:0")]),
            "private show server arguments were {arguments:?}"
        );
    }
    #[test]
    fn preview_adoption_preserves_original_identity_and_uncheckpointed_wal() {
        let directory = scratch_directory(Path::new("snapshot-regression"))
            .unwrap()
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&directory).unwrap();
        let original = directory.join("Original.show");
        let writer = rusqlite::Connection::open(&original).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT); INSERT INTO metadata VALUES ('show_id','parent-index-identity'); CREATE TABLE fixture(value TEXT); INSERT INTO fixture VALUES ('uncheckpointed fixture');").unwrap();
        let database_before = std::fs::read(&original).unwrap();
        let wal_path = directory.join("Original.show-wal");
        let wal_before = std::fs::read(&wal_path).unwrap();
        let workspace = PreviewWorkspace::create(&directory, &original).unwrap();
        let command = private_server_command(
            Path::new("light-headless"),
            &workspace.directory,
            &workspace.snapshot,
            5311,
        );
        let arguments = command.get_args().collect::<Vec<_>>();
        let show_index = arguments
            .iter()
            .position(|argument| *argument == OsStr::new("--show"))
            .unwrap();
        let private = rusqlite::Connection::open(Path::new(arguments[show_index + 1])).unwrap();
        assert_eq!(
            private
                .query_row("SELECT value FROM fixture", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "uncheckpointed fixture"
        );
        private
            .execute(
                "UPDATE metadata SET value='private-index-identity' WHERE key='show_id'",
                [],
            )
            .unwrap();
        assert_eq!(
            writer
                .query_row(
                    "SELECT value FROM metadata WHERE key='show_id'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "parent-index-identity"
        );
        assert_eq!(std::fs::read(&original).unwrap(), database_before);
        assert_eq!(std::fs::read(&wal_path).unwrap(), wal_before);
        drop(private);
        drop(workspace);
        drop(writer);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn repeated_openings_and_failed_startup_preserve_original_and_other_previews() {
        let parent = scratch_directory(Path::new("snapshot-isolation-regression"))
            .unwrap()
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&parent).unwrap();
        let source = parent.join("Original.show");
        let connection = rusqlite::Connection::open(&source).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE metadata(value TEXT); INSERT INTO metadata VALUES('original');",
            )
            .unwrap();
        drop(connection);
        let original = std::fs::read(&source).unwrap();
        let first = PreviewWorkspace::create(&parent, &source).unwrap();
        let second = PreviewWorkspace::create(&parent, &source).unwrap();
        assert_ne!(first.directory, second.directory);
        assert_ne!(first.snapshot, source);
        assert_eq!(first.snapshot.file_name(), source.file_name());
        let failed_directory = first.directory.clone();
        assert!(first.start(&parent.join("missing-headless"), 5311).is_err());
        drop(first);
        assert!(!failed_directory.exists());
        assert!(second.snapshot.is_file());
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let second_directory = second.directory.clone();
        drop(second);
        assert!(!second_directory.exists());
        assert_eq!(std::fs::read(&source).unwrap(), original);
        let malformed = parent.join("Malformed.show");
        std::fs::write(&malformed, b"not a SQLite show").unwrap();
        let before = std::fs::read_dir(&parent).unwrap().count();
        assert!(PreviewWorkspace::create(&parent, &malformed).is_err());
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), before);
        assert_eq!(std::fs::read(&malformed).unwrap(), b"not a SQLite show");
        std::fs::remove_dir_all(parent).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn exited_private_server_retains_original_label_and_cleans_only_its_snapshot() {
        let parent = scratch_directory(Path::new("snapshot-exited-regression"))
            .unwrap()
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&parent).unwrap();
        let original = parent.join("Operator show.show");
        let connection = rusqlite::Connection::open(&original).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE metadata(value TEXT); INSERT INTO metadata VALUES('original');",
            )
            .unwrap();
        drop(connection);
        let bytes = std::fs::read(&original).unwrap();
        let workspace = PreviewWorkspace::create(&parent, &original).unwrap();
        let directory = workspace.directory.clone();
        let mut child = workspace.start(Path::new("/usr/bin/false"), 5311).unwrap();
        assert!(!child.wait().unwrap().success());
        let mut hosted = HostedShow {
            child,
            port: 5311,
            path: original.clone(),
            _workspace: workspace,
        };
        assert_eq!(hosted.path(), original);
        assert_eq!(hosted.label(), "Operator show.show");
        assert!(hosted.exited());
        drop(hosted);
        assert!(!directory.exists());
        assert_eq!(std::fs::read(&original).unwrap(), bytes);
        std::fs::remove_dir_all(parent).unwrap();
    }
}
