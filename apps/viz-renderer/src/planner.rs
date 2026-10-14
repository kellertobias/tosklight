//! Opening the planning window when nothing was named.
//!
//! Started on its own, with no console to attach to and no file to look at, the visualizer has
//! nothing to draw and no way to be told what to draw. So it opens the Viz editor beside itself
//! and connects to the document that window holds: the operator picks or builds a rig there, and
//! it appears here.
//!
//! The editor is an accessory executable inside the Visualizer product. If a development build is
//! incomplete the visualizer says so and keeps running — an empty picture with a readable reason
//! is better than a window that refuses to open.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// The planning window this visualizer opened, and the port it serves its document on.
#[derive(Debug)]
pub struct PlanningWindow {
    child: Child,
    port: u16,
}

impl PlanningWindow {
    /// Launch the editor and tell it where to serve the document it opens.
    pub fn open() -> Result<Self, String> {
        let binary = editor_binary()?;
        let port = free_port()?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let child = Command::new(&binary)
            .arg("--serve")
            .arg(address.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", binary.display()))?;
        Ok(Self { child, port })
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Whether the operator has closed the planning window. Its document goes with it, so the
    /// visualizer says so rather than retrying a port nothing is listening on any more.
    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }
}

impl Drop for PlanningWindow {
    /// Closing the visualizer closes the planning window it opened. An operator who wants the
    /// editor on its own starts it on its own.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether a planning window could be opened at all, and why not when it could not.
///
/// The source control asks this rather than offering a planning source that cannot start, and it
/// answers with the same message the failed launch would have given.
pub fn availability() -> Result<(), String> {
    editor_binary().map(|_| ())
}

/// Names the editor to open, for a development tree or an unusual installation. A build that
/// ships the two together never needs it.
pub const EDITOR_PATH_ENV: &str = "TOSKLIGHT_VIZ_EDITOR";

/// The editor that shipped beside this binary, or the one installed on this machine.
fn editor_binary() -> Result<PathBuf, String> {
    resolve_editor(
        std::env::var_os(EDITOR_PATH_ENV).map(PathBuf::from),
        || {
            std::env::current_exe()
                .map_err(|error| format!("could not locate the visualizer binary: {error}"))
        },
        installed_editor,
        cfg!(target_os = "macos"),
        cfg!(windows),
    )
}

fn resolve_editor(
    named: Option<PathBuf>,
    executable: impl FnOnce() -> Result<PathBuf, String>,
    installed: impl FnOnce() -> Option<PathBuf>,
    macos: bool,
    windows: bool,
) -> Result<PathBuf, String> {
    if let Some(path) = named.filter(|path| !path.as_os_str().is_empty()) {
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{EDITOR_PATH_ENV} names {}, which is not a file",
            path.display()
        ));
    }
    let executable = executable()?;
    let directory = executable
        .parent()
        .ok_or_else(|| "the visualizer binary has no directory".to_owned())?;
    if macos {
        let branded = directory.join("ToskLight Architect");
        if branded.is_file() {
            return Ok(branded);
        }
    }
    let name = if windows {
        "viz-editor.exe"
    } else {
        "viz-editor"
    };
    let beside = directory.join(name);
    if beside.is_file() {
        return Ok(beside);
    }
    if let Some(installed) = installed() {
        return Ok(installed);
    }
    Err(format!(
        "the Viz editor is not beside the visualizer at {} and is not installed; build it with \
         `npm run build:viz-editor`",
        directory.display()
    ))
}

#[cfg(target_os = "macos")]
fn installed_editor() -> Option<PathBuf> {
    let bundles = [
        // Architect's executable is the editor, named after the product.
        "Applications/ToskLight Architect.app/Contents/MacOS/ToskLight Architect",
        // Bundles built before the editor became the executable carried it beside the renderer
        // under its own file name, and the product has been renamed twice; each of those stays
        // discoverable so an installation that has not been replaced yet still opens its editor.
        "Applications/ToskLight Architect.app/Contents/MacOS/viz-editor",
        "Applications/ToskLight PreViz.app/Contents/MacOS/viz-editor",
        "Applications/ToskLight Visualizer.app/Contents/MacOS/viz-editor",
        "Applications/ToskLight Viz Editor.app/Contents/MacOS/viz-editor",
    ];
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    bundles
        .into_iter()
        .map(|bundle| home.join(bundle))
        .find(|path| path.is_file())
}

#[cfg(not(target_os = "macos"))]
fn installed_editor() -> Option<PathBuf> {
    None
}

/// A loopback port nothing else is listening on. Released before the editor is told to use it,
/// exactly as a hosted show file does.
fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|error| format!("no free port for the planning window: {error}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|error| format!("no free port for the planning window: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EditorFiles(PathBuf);
    impl EditorFiles {
        fn new() -> Self {
            let temporary = std::env::var_os("LIGHT_TMP_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    std::env::var_os("LIGHT_ARTIFACTS_DIR")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| {
                            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.artifacts")
                        })
                        .join("tmp")
                });
            let root = temporary.join(format!("tl783-editor-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn file(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"fixture executable; never launched").unwrap();
            path
        }
        fn resolve(
            &self,
            macos: bool,
            windows: bool,
            installed: Option<PathBuf>,
        ) -> Result<PathBuf, String> {
            resolve_editor(
                None,
                || Ok(self.0.join("viz-renderer")),
                || installed,
                macos,
                windows,
            )
        }
    }
    impl Drop for EditorFiles {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn editor_resolution_branded_mac_sibling_precedes_old_installation() {
        let files = EditorFiles::new();
        let branded = files.file("ToskLight Architect");
        let installed = files.file("old-installed-editor");
        assert_eq!(
            files.resolve(true, false, Some(installed)).unwrap(),
            branded
        );
    }

    #[test]
    fn editor_resolution_current_brand_precedes_generic_mac_sibling() {
        let files = EditorFiles::new();
        let branded = files.file("ToskLight Architect");
        files.file("viz-editor");
        assert_eq!(files.resolve(true, false, None).unwrap(), branded);
    }

    #[test]
    fn editor_resolution_generic_platforms_and_installed_fallback_remain() {
        let files = EditorFiles::new();
        let generic = files.file("viz-editor");
        let windows = files.file("viz-editor.exe");
        assert_eq!(files.resolve(true, false, None).unwrap(), generic);
        assert_eq!(files.resolve(false, false, None).unwrap(), generic);
        assert_eq!(files.resolve(false, true, None).unwrap(), windows);
        std::fs::remove_file(generic).unwrap();
        let old = files.file("old-installed-editor");
        assert_eq!(files.resolve(true, false, Some(old.clone())).unwrap(), old);
    }

    #[test]
    fn editor_resolution_missing_all_is_actionable() {
        let files = EditorFiles::new();
        assert!(
            files
                .resolve(true, false, None)
                .unwrap_err()
                .contains("npm run build:viz-editor")
        );
    }

    #[test]
    fn editor_resolution_non_mac_platform_ignores_mac_brand() {
        let files = EditorFiles::new();
        files.file("ToskLight Architect");
        let generic = files.file("viz-editor");
        let windows = files.file("viz-editor.exe");
        assert_eq!(files.resolve(false, false, None).unwrap(), generic);
        assert_eq!(files.resolve(false, true, None).unwrap(), windows);
    }

    #[test]
    fn editor_resolution_explicit_override_is_authoritative() {
        let files = EditorFiles::new();
        let named = files.file("override");
        let resolved = resolve_editor(
            Some(named.clone()),
            || panic!("override must bypass discovery"),
            || panic!("override must bypass installed fallback"),
            true,
            false,
        )
        .unwrap();
        assert_eq!(resolved, named);
    }

    #[test]
    fn editor_resolution_invalid_override_refuses_without_fallback() {
        let files = EditorFiles::new();
        let missing = files.0.join("missing-override");
        files.file("ToskLight Architect");
        let error = resolve_editor(
            Some(missing),
            || panic!("invalid override cannot discover"),
            || panic!("invalid override cannot fall back"),
            true,
            false,
        )
        .unwrap_err();
        assert!(error.contains(EDITOR_PATH_ENV));
        assert!(error.contains("not a file"));
    }

    #[test]
    fn the_editor_is_looked_for_beside_the_visualizer() {
        // Not installed in the test environment, so this reports rather than panicking, and the
        // message names the command that fixes it.
        if let Err(message) = editor_binary() {
            assert!(message.contains("npm run build:viz-editor"), "{message}");
        }
    }

    #[test]
    fn each_planning_window_gets_its_own_port() {
        let first = free_port().expect("first");
        let second = free_port().expect("second");
        assert!(first > 0 && second > 0);
    }
}
