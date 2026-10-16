//! Private benchmark capture only. Raw stderr and child identity never enter the wire protocol.
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);
const ATTEMPTS: usize = 16;
const FLAG: &str = "LIGHT_VIZ_TYPED_CPU_DIAGNOSTICS";
const DIRECTORY: &str = "LIGHT_VIZ_TYPED_CPU_CAPTURE_DIR";

fn effective(environment: &[(String, String)], key: &str) -> Option<OsString> {
    environment
        .iter()
        .rev()
        .find(|(name, _)| name == key)
        .map(|(_, value)| OsString::from(value))
        .or_else(|| std::env::var_os(key))
}

pub(super) fn prepare(environment: &[(String, String)]) -> Result<Option<Capture>, String> {
    if effective(environment, FLAG).as_deref() != Some(std::ffi::OsStr::new("1")) {
        return Ok(None);
    }
    let directory = effective(environment, DIRECTORY).ok_or("capture directory is required")?;
    Capture::reserve(Path::new(&directory))
        .map(Some)
        .map_err(|error| error.to_string())
}

pub(super) struct Capture {
    stderr: Option<File>,
    manifest: File,
    stderr_path: PathBuf,
    manifest_path: PathBuf,
    ordinal: u64,
    bound: bool,
}

#[cfg(unix)]
fn private_directory(path: &Path) -> std::io::Result<fs::Metadata> {
    use std::os::unix::fs::PermissionsExt;
    if !path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(std::io::Error::other("capture directory must be absolute"));
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(std::io::Error::other(
                "capture directory ancestors must be real directories",
            ));
        }
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.permissions().mode() & 0o7777 != 0o700 {
        return Err(std::io::Error::other(
            "capture directory must already have mode0700",
        ));
    }
    Ok(metadata)
}

#[cfg(unix)]
fn exclusive(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(unix)]
impl Capture {
    fn reserve(directory: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let identity = private_directory(directory)?;
        for _ in 0..ATTEMPTS {
            let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
            if ordinal == u64::MAX {
                return Err(std::io::Error::other("capture ordinal exhausted"));
            }
            let stem = format!("viz-cpu-{}-{ordinal}", std::process::id());
            let stderr_path = directory.join(format!("{stem}.stderr"));
            let manifest_path = directory.join(format!("{stem}.json"));
            let stderr = match exclusive(&stderr_path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            };
            let manifest = match exclusive(&manifest_path) {
                Ok(file) => file,
                Err(error) => {
                    fs::remove_file(&stderr_path)?;
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        continue;
                    }
                    return Err(error);
                }
            };
            let capture = Self {
                stderr: Some(stderr),
                manifest,
                stderr_path,
                manifest_path,
                ordinal,
                bound: false,
            };
            let actual = capture.manifest.metadata()?;
            let stderr_metadata = capture
                .stderr
                .as_ref()
                .expect("reserved stderr")
                .metadata()?;
            let now = private_directory(directory)?;
            // A newly created file's owner is the effective user. This also verifies directory
            // ownership without another process, unsafe libc, or trusting an environment UID.
            if stderr_metadata.uid() != identity.uid()
                || stderr_metadata.permissions().mode() & 0o7777 != 0o600
                || actual.uid() != identity.uid()
                || actual.permissions().mode() & 0o7777 != 0o600
                || identity.dev() != now.dev()
                || identity.ino() != now.ino()
            {
                return Err(std::io::Error::other(
                    "capture directory ownership or identity changed",
                ));
            }
            return Ok(capture);
        }
        Err(std::io::Error::other(
            "exclusive capture collision budget exhausted",
        ))
    }
}

#[cfg(not(unix))]
impl Capture {
    fn reserve(_: &Path) -> std::io::Result<Self> {
        Err(std::io::Error::other(
            "private CPU capture requires Unix permission checks",
        ))
    }
}

impl Capture {
    pub(super) fn stderr(&mut self) -> Stdio {
        Stdio::from(
            self.stderr
                .take()
                .expect("capture stderr is handed out once"),
        )
    }
    pub(super) fn bind(
        &mut self,
        child_pid: u32,
        program: &Path,
        arguments: &[String],
    ) -> Result<(), String> {
        #[derive(serde::Serialize)]
        struct Binding<'a> {
            private_capture_schema: u8,
            child_pid: u32,
            parent_pid: u32,
            spawn_ordinal: u64,
            program: &'a Path,
            arguments: &'a [String],
            stderr_path: &'a Path,
        }
        let manifest = Binding {
            private_capture_schema: 1,
            child_pid,
            parent_pid: std::process::id(),
            spawn_ordinal: self.ordinal,
            program,
            arguments,
            stderr_path: &self.stderr_path,
        };
        let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
        self.manifest
            .write_all(&bytes)
            .and_then(|_| self.manifest.sync_all())
            .map_err(|error| error.to_string())?;
        self.bound = true;
        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        if !self.bound {
            let _ = fs::remove_file(&self.stderr_path);
            let _ = fs::remove_file(&self.manifest_path);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{HelperState, SupervisedHelper};
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let base = std::env::var_os("LIGHT_TMP_DIR").map(PathBuf::from).unwrap_or_else(|| {
                let repository = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap();
                let result = std::process::Command::new("bash")
                    .args(["-c", "source tools/artifact-paths.sh; light_init_artifact_paths \"$PWD\"; printf '%s' \"$LIGHT_TMP_DIR\""])
                    .current_dir(repository).output().expect("canonical artifact resolver runs");
                assert!(result.status.success(), "canonical artifact resolver failed");
                PathBuf::from(String::from_utf8(result.stdout).unwrap())
            });
            fs::create_dir_all(&base).unwrap();
            let path = base.join(format!(
                "helper-cpu-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self(path)
        }
        fn environment(&self, flag: &str) -> Vec<(String, String)> {
            vec![
                (FLAG.into(), flag.into()),
                (DIRECTORY.into(), self.0.to_str().unwrap().into()),
            ]
        }
        fn helper(&self) -> SupervisedHelper {
            let mut helper = SupervisedHelper::new(
                "/bin/sh",
                vec![
                    "-c".into(),
                    "printf 'private sample\\n' >&2; exec /bin/cat".into(),
                ],
            );
            helper.environment = self.environment("1");
            helper
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn off_and_effective_last_override_create_no_files() {
        let directory = Directory::new();
        for flag in ["0", "true", ""] {
            assert!(prepare(&directory.environment(flag)).unwrap().is_none());
        }
        let mut overrides = directory.environment("1");
        overrides.push((FLAG.into(), "0".into()));
        assert!(prepare(&overrides).unwrap().is_none());
        let mut helper = directory.helper();
        helper.environment = overrides;
        helper.start().unwrap();
        helper.stop();
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
    #[test]
    fn two_children_and_restart_have_distinct_private_bindings_and_unchanged_stdout() {
        let directory = Directory::new();
        let mut one = directory.helper();
        let mut two = directory.helper();
        one.start().unwrap();
        two.start().unwrap();
        let first = one.child.as_ref().unwrap().id();
        let second = two.child.as_ref().unwrap().id();
        for helper in [&mut one, &mut two] {
            let (mut input, mut output) = helper.take_channel().unwrap();
            crate::framing::write_frame(&mut input, b"\0wire\xff").unwrap();
            assert_eq!(
                crate::framing::read_frame(&mut output).unwrap(),
                b"\0wire\xff"
            );
        }
        one.start().unwrap();
        let restarted = one.child.as_ref().unwrap().id();
        let (mut input, mut output) = one.take_channel().unwrap();
        crate::framing::write_frame(&mut input, b"restart wire").unwrap();
        assert_eq!(
            crate::framing::read_frame(&mut output).unwrap(),
            b"restart wire"
        );
        one.stop();
        two.stop();
        let mut pids = Vec::new();
        let mut ordinals = Vec::new();
        for entry in fs::read_dir(&directory.0).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
                0o600
            );
            if path.extension().unwrap() == "json" {
                let binding: serde_json::Value =
                    serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
                pids.push(binding["child_pid"].as_u64().unwrap() as u32);
                ordinals.push(binding["spawn_ordinal"].as_u64().unwrap());
                assert_eq!(binding["parent_pid"], std::process::id());
                assert_eq!(binding["program"], "/bin/sh");
                assert!(
                    fs::read_to_string(binding["stderr_path"].as_str().unwrap())
                        .unwrap()
                        .contains("private sample")
                );
            }
        }
        pids.sort();
        let mut expected = vec![first, second, restarted];
        expected.sort();
        assert_eq!(pids, expected);
        ordinals.sort();
        ordinals.dedup();
        assert_eq!(ordinals.len(), 3);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 6);
    }
    #[test]
    fn exclusive_file_and_symlink_targets_are_never_overwritten() {
        let directory = Directory::new();
        let target = directory.0.join("existing");
        fs::write(&target, b"keep").unwrap();
        assert_eq!(
            exclusive(&target).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        let link = directory.0.join("link");
        symlink(&target, &link).unwrap();
        assert_eq!(
            exclusive(&link).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert_eq!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            true
        );
    }
    #[test]
    fn invalid_setup_stops_without_spawning_or_silent_fallback() {
        let directory = Directory::new();
        let mut helper = directory.helper();
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(helper.start().unwrap_err().starts_with("STOP:"));
        assert!(helper.child.is_none());
        assert!(matches!(helper.state(), HelperState::GaveUp { .. }));
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
        helper.poll(std::time::Instant::now() + std::time::Duration::from_secs(100));
        assert!(helper.child.is_none());
    }
    #[test]
    fn manifest_write_failure_reaps_only_the_owned_child_and_removes_unbound_files() {
        let directory = Directory::new();
        let mut capture = Capture::reserve(&directory.0).unwrap();
        let manifest_path = capture.manifest_path.clone();
        capture.manifest = File::open(&manifest_path).unwrap(); // deterministic write failure
        let mut owner = SupervisedHelper::new("/bin/sleep", vec!["30".into()]);
        let mut other = SupervisedHelper::new("/bin/sleep", vec!["30".into()]);
        owner.environment = directory.environment("0");
        other.environment = directory.environment("0");
        owner.start().unwrap();
        other.start().unwrap();
        assert!(
            owner
                .bind_capture(&mut capture)
                .unwrap_err()
                .starts_with("STOP:")
        );
        assert!(owner.child.is_none());
        assert!(owner.restart_at.is_none());
        assert!(other.child.as_mut().unwrap().try_wait().unwrap().is_none());
        drop(capture);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }
    #[test]
    fn relative_missing_and_symlink_directories_are_refused() {
        assert!(Capture::reserve(Path::new("relative")).is_err());
        let directory = Directory::new();
        let link = directory.0.join("linked");
        symlink(&directory.0, &link).unwrap();
        assert!(Capture::reserve(&link).is_err());
        let mut env = directory.environment("1");
        env[1].1 = directory.0.join("missing").to_str().unwrap().into();
        assert!(prepare(&env).is_err());
    }
}
