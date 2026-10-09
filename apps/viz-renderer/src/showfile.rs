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
    let base = scratch_base(
        std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from),
        std::env::current_exe().ok().as_deref(),
        std::env::consts::OS,
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from),
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from),
        std::env::temp_dir(),
    )?;
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

/// A caller-specified scratch destination wins. Bare development binaries recognize their
/// actual repository artifact ancestry; installed binaries use writable user-owned cache state,
/// never the launch working directory or a compile-time checkout path.
fn scratch_base(
    override_path: Option<PathBuf>,
    executable: Option<&Path>,
    platform: &str,
    home: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    xdg_cache: Option<PathBuf>,
    temporary: PathBuf,
) -> Result<PathBuf, String> {
    if let Some(path) = override_path.filter(|path| !path.as_os_str().is_empty()) {
        return Ok(path);
    }
    if let Some(path) = executable.and_then(repository_scratch_base) {
        return Ok(path);
    }
    let home = home.filter(|path| path.is_absolute());
    let cache = match platform {
        "macos" => home.map(|path| path.join("Library/Caches/ToskLight/Visualizer")),
        "windows" => local_app_data
            .filter(|path| path.is_absolute())
            .map(|path| path.join("ToskLight/Visualizer/Cache")),
        _ => xdg_cache
            .filter(|path| path.is_absolute())
            .or_else(|| home.map(|path| path.join(".cache")))
            .map(|path| path.join("tosklight/visualizer")),
    };
    if let Some(path) = cache {
        return Ok(path);
    }
    if temporary.is_absolute() {
        return Ok(temporary.join("tosklight/visualizer"));
    }
    Err(
        "no absolute writable scratch root is available; set LIGHT_TMP_DIR to a writable directory"
            .to_owned(),
    )
}

fn repository_scratch_base(executable: &Path) -> Option<PathBuf> {
    if !executable.is_absolute() {
        return None;
    }
    let artifact = executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == ".artifacts"))?;
    let repository = artifact.parent()?;
    // A coincidentally named folder in an installed application is not a checkout. Require
    // the actual workspace, renderer manifest and canonical artifact layout beside it.
    let workspace = std::fs::read_to_string(repository.join("Cargo.toml")).ok()?;
    let renderer = std::fs::read_to_string(repository.join("apps/viz-renderer/Cargo.toml")).ok()?;
    let layout = std::fs::read_to_string(repository.join("tools/artifact-layout.conf")).ok()?;
    if !workspace.lines().any(|line| line.trim() == "[workspace]")
        || !renderer
            .lines()
            .any(|line| line.trim() == "name = \"viz-renderer\"")
    {
        return None;
    }
    let relative = Path::new(
        layout
            .lines()
            .find_map(|line| line.strip_prefix("TMP_ROOT="))?,
    );
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(artifact.join(relative))
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
    #[test]
    fn standalone_scratch_without_wrapper_is_absolute_and_app_owned() {
        let root = std::env::current_dir().unwrap();
        let home = root.join(".artifacts/tmp/fake-user");
        let temporary = root.join(".artifacts/tmp/fake-os-temp");
        let installed = root.join("Installed.app/Contents/MacOS/viz-renderer");
        let resolved = scratch_base(
            None,
            Some(&installed),
            "macos",
            Some(home.clone()),
            None,
            None,
            temporary,
        )
        .unwrap();
        assert!(
            resolved.is_absolute(),
            "standalone scratch must not depend on launch cwd: {}",
            resolved.display()
        );
        assert_eq!(resolved, home.join("Library/Caches/ToskLight/Visualizer"));
    }

    #[test]
    fn scratch_roots_preserve_overrides_and_platform_defaults_without_cwd() {
        let root = std::env::current_dir()
            .unwrap()
            .join(".artifacts/tmp/scratch-root-inputs");
        let home = root.join("user");
        let app_data = root.join("local-data");
        let cache = root.join("xdg");
        let temp = root.join("temp");
        let resolve = |platform, home, local, xdg| {
            scratch_base(None, None, platform, home, local, xdg, temp.clone()).unwrap()
        };
        assert_eq!(
            resolve("windows", Some(home.clone()), Some(app_data.clone()), None),
            app_data.join("ToskLight/Visualizer/Cache")
        );
        assert_eq!(
            resolve("linux", Some(home.clone()), None, Some(cache.clone())),
            cache.join("tosklight/visualizer")
        );
        assert_eq!(
            resolve(
                "linux",
                Some(home.clone()),
                None,
                Some(PathBuf::from("relative-cache"))
            ),
            home.join(".cache/tosklight/visualizer")
        );
        assert_eq!(
            resolve("macos", None, None, None),
            temp.join("tosklight/visualizer")
        );
        assert_eq!(
            resolve("windows", None, Some(PathBuf::from("relative")), None),
            temp.join("tosklight/visualizer")
        );
        for explicit in [
            root.join("explicit"),
            PathBuf::from("deliberate-relative-override"),
        ] {
            assert_eq!(
                scratch_base(
                    Some(explicit.clone()),
                    None,
                    "macos",
                    Some(home.clone()),
                    None,
                    None,
                    temp.clone()
                )
                .unwrap(),
                explicit
            );
        }
        assert_eq!(
            scratch_base(
                Some(PathBuf::new()),
                None,
                "macos",
                None,
                None,
                None,
                temp.clone()
            )
            .unwrap(),
            temp.join("tosklight/visualizer")
        );
        assert!(
            scratch_base(
                None,
                None,
                "macos",
                Some(PathBuf::from("relative-user")),
                None,
                None,
                PathBuf::from("relative-temp")
            )
            .unwrap_err()
            .contains("LIGHT_TMP_DIR")
        );
    }

    #[test]
    fn repository_scratch_requires_real_artifact_ancestry_and_checkout_contract() {
        let parent = scratch_directory(Path::new("repo-scratch-contract"))
            .unwrap()
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&parent).unwrap();
        let executable = parent.join(".artifacts/build/cargo/release/viz-renderer");
        assert!(repository_scratch_base(&executable).is_none());
        std::fs::create_dir_all(parent.join("tools")).unwrap();
        std::fs::create_dir_all(parent.join("apps/viz-renderer")).unwrap();
        std::fs::write(parent.join("Cargo.toml"), "[workspace]\n").unwrap();
        std::fs::write(
            parent.join("apps/viz-renderer/Cargo.toml"),
            "[package]\nname = \"viz-renderer\"\n",
        )
        .unwrap();
        std::fs::write(parent.join("tools/artifact-layout.conf"), "TMP_ROOT=tmp\n").unwrap();
        assert_eq!(
            repository_scratch_base(&executable),
            Some(parent.join(".artifacts/tmp"))
        );
        let installed = parent
            .ancestors()
            .last()
            .unwrap()
            .join("Applications/Installed.app/Contents/MacOS/viz-renderer");
        assert!(repository_scratch_base(&installed).is_none());
        std::fs::write(
            parent.join("tools/artifact-layout.conf"),
            "TMP_ROOT=../escape\n",
        )
        .unwrap();
        assert!(repository_scratch_base(&executable).is_none());
        std::fs::remove_dir_all(parent).unwrap();
    }
}
