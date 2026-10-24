use super::{UltimateError, process};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant, SystemTime},
};
const MARKER: &str = "TOSKLIGHT_BLENDER_PROBE=";
const SCRIPT: &str = r#"import bpy,json
usable=False
for space,name in [(bpy.ops.import_scene,'gltf'),(bpy.ops.wm,'gltf_import')]:
    try:
        getattr(space,name).get_rna_type()
        usable=True
        break
    except Exception:
        pass
bpy.context.scene.render.engine='CYCLES'
print('TOSKLIGHT_BLENDER_PROBE='+json.dumps({'version':bpy.app.version_string,'gltf':usable,'cycles':hasattr(bpy.context.scene,'cycles')}),flush=True)
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    permissions: std::fs::Permissions,
}
impl Identity {
    fn read(path: &Path) -> Result<Self, UltimateError> {
        let path = path.canonicalize()?;
        let metadata = path.metadata()?;
        if !metadata.is_file() {
            return Err(UltimateError::Failed(
                "Configured Blender is not a file".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(UltimateError::Failed(
                    "Configured Blender is not executable; choose Ultra or fix its permissions"
                        .into(),
                ));
            }
        }
        Ok(Self {
            path,
            length: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            permissions: metadata.permissions(),
        })
    }
}
#[derive(Clone, Debug)]
pub struct BlenderCapabilities {
    pub executable: PathBuf,
    pub version: String,
    identity: Identity,
}
impl BlenderCapabilities {
    pub(super) fn verify_identity(&self) -> Result<(), UltimateError> {
        if Identity::read(&self.executable)? != self.identity {
            return Err(UltimateError::Failed(
                "Blender changed after the quality choice; check it again or choose Ultra".into(),
            ));
        }
        Ok(())
    }
}
struct Entry {
    identity: Identity,
    at: Instant,
    result: Result<BlenderCapabilities, UltimateError>,
}
#[derive(Default)]
pub struct BlenderProbeCache {
    entries: VecDeque<Entry>,
}

impl BlenderProbeCache {
    /// Call on a worker after freezing the look. Cache is bounded to eight executable identities
    /// and expires in 30 seconds so importer/Cycles installation changes are not held indefinitely.
    pub fn probe(
        &mut self,
        configured: Option<&Path>,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<BlenderCapabilities, UltimateError> {
        super::check_cancel(cancel)?;
        let path = crate::find_blender(configured).ok_or_else(|| {
            UltimateError::Failed(
                "Blender not found; Ultra is available without installing Blender".into(),
            )
        })?;
        let path = super::launcher::resolve_image_launcher(&path)?;
        let identity = Identity::read(&path)?;
        if let Some(entry) = self.entries.iter().find(|entry| {
            entry.identity == identity && entry.at.elapsed() < Duration::from_secs(30)
        }) {
            return entry.result.clone();
        }
        let result = probe(&identity, timeout.min(Duration::from_secs(30)), cancel);
        if !matches!(
            result,
            Err(UltimateError::Cancelled | UltimateError::TimedOut)
        ) {
            self.entries.retain(|entry| entry.identity != identity);
            if self.entries.len() == 8 {
                self.entries.pop_front();
            }
            self.entries.push_back(Entry {
                identity,
                at: Instant::now(),
                result: result.clone(),
            });
        }
        result
    }
}
fn probe(
    identity: &Identity,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<BlenderCapabilities, UltimateError> {
    let mut command = tokio::process::Command::new(&identity.path);
    let script = format!(
        "{}\ninstall_parent_guard()\n{}",
        include_str!("parent_guard.py"),
        SCRIPT
    );
    command.args([
        "--background",
        "--factory-startup",
        "--python-exit-code",
        "13",
        "--python-expr",
        &script,
    ]);
    let outcome = process::run(command, timeout, cancel, |_| {})?;
    if !outcome.status.success() {
        return Err(UltimateError::Failed(format!(
            "Blender is unusable: {}; choose Ultra",
            outcome.diagnostic()
        )));
    }
    #[derive(Deserialize)]
    struct Probe {
        version: String,
        gltf: bool,
        cycles: bool,
    }
    let decoded = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix(MARKER))
        .ok_or_else(|| {
            UltimateError::Failed("Blender returned no capability result; choose Ultra".into())
        })?;
    let probe: Probe = serde_json::from_str(decoded).map_err(|error| {
        UltimateError::Failed(format!("Invalid Blender capability result: {error}"))
    })?;
    if !probe.gltf || !probe.cycles {
        return Err(UltimateError::Failed(format!(
            "Blender lacks {}{}; choose Ultra",
            if probe.gltf { "" } else { "glTF importer " },
            if probe.cycles { "" } else { "Cycles" }
        )));
    }
    Ok(BlenderCapabilities {
        executable: identity.path.clone(),
        version: probe.version.chars().take(128).collect(),
        identity: identity.clone(),
    })
}
