//! Opening a visualizer the desk owns, without taking on its risk.
//!
//! Hardware Controls is opened as a sibling application because it is one: its own product, its
//! own Dock tile, its own life. The visualizer is not. It is this desk's window, shipped inside
//! this bundle, and it must not become a second application in the App Switcher — so it is started
//! as a supervised helper rather than handed to the platform to launch.
//!
//! The supervision is the point. A renderer draws through a GPU driver, and a driver can end a
//! process in ways no care in our own code prevents. Run in-process it would take the Programmer,
//! playback and the output engine with it. Run as a child it takes its own window and nothing
//! else, and the desk says what happened.

use std::path::PathBuf;
use std::sync::Mutex;
use viz_helper::{HelperState, SupervisedHelper};

/// The visualizer this desk has open, if any.
///
/// One at a time: a second would be a second window drawing the same show, which is not a feature
/// anybody asked for and is two GPU contexts for one picture.
#[derive(Default)]
pub(crate) struct Visualizer {
    helper: Mutex<Option<SupervisedHelper>>,
}

impl Visualizer {
    /// Start it, replacing one already running.
    pub(crate) fn open(&self) -> Result<(), String> {
        self.close()?;
        let program = helper_binary()?;
        let address = crate::server::address();
        // The renderer's desk provider is the authoritative scene path: it reads the active show and
        // follows show events itself, while its normal DMX receivers keep live values current.
        let mut helper = SupervisedHelper::new(program, desk_arguments(address))
            .with_environment("TOSKLIGHT_VIZ_LAUNCHED_BY", "desk");
        helper.start()?;
        *self.helper.lock().map_err(|_| "visualizer state")? = Some(helper);
        Ok(())
    }

    /// What is drawing, once the helper has said. `None` before the greeting completes.
    pub(crate) fn renderer(&self) -> Result<Option<String>, String> {
        Ok(self.is_open()?.then(|| "ToskLight PreViz".to_owned()))
    }

    pub(crate) fn close(&self) -> Result<(), String> {
        if let Some(mut helper) = self.helper.lock().map_err(|_| "visualizer state")?.take() {
            helper.stop();
        }
        Ok(())
    }

    /// Notice a helper that has died, and restart or give up. Driven from the desk's own loop so
    /// supervision cannot race with the desk deciding to close it.
    pub(crate) fn poll(&self) -> Result<(), String> {
        let mut helpers = self.helper.lock().map_err(|_| "visualizer state")?;
        let Some(helper) = helpers.as_mut() else {
            return Ok(());
        };
        helper.poll(std::time::Instant::now());
        Ok(())
    }

    pub(crate) fn state(&self) -> Result<HelperState, String> {
        Ok(self
            .helper
            .lock()
            .map_err(|_| "visualizer state")?
            .as_ref()
            .map_or(HelperState::Down, |helper| helper.state().clone()))
    }

    pub(crate) fn is_open(&self) -> Result<bool, String> {
        Ok(self
            .helper
            .lock()
            .map_err(|_| "visualizer state")?
            .is_some())
    }
}

fn desk_arguments(address: std::net::SocketAddr) -> Vec<String> {
    vec![
        "--server".to_owned(),
        address.ip().to_string(),
        "--port".to_owned(),
        address.port().to_string(),
    ]
}

/// The renderer shipped inside this bundle.
///
/// Never one found on the machine at large: a visualizer from another install would be another
/// build, and this desk supervises it and speaks a versioned protocol to it. The helper is ours or
/// there is not one.
pub(crate) fn helper_binary() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let beside = executable.with_file_name(renderer_binary_name());
    if beside.is_file() {
        return Ok(beside);
    }
    // A development tree has the two beside each other in the target directory rather than inside
    // one bundle, which is the case `npm run build:open` produces.
    if let Some(target) = executable
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "cargo"))
    {
        for profile in ["release", "debug"] {
            let candidate = target.join(profile).join(renderer_binary_name());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "the visualizer is not beside this application at {}; build it with `npm run build:viz`",
        beside.display()
    ))
}

#[cfg(windows)]
const fn renderer_binary_name() -> &'static str {
    "viz-renderer.exe"
}

#[cfg(not(windows))]
const fn renderer_binary_name() -> &'static str {
    "viz-renderer"
}

/// Open the visualizer, or report why it could not be opened.
#[tauri::command]
pub(crate) fn open_visualizer(
    visualizer: tauri::State<'_, Visualizer>,
    panes: tauri::State<'_, crate::stage_pane::StagePanes>,
) -> Result<(), String> {
    if panes.has_live_3d()? {
        return Err(
            "Only one live 3D Stage is supported. Close the embedded or external 3D Stage before opening the Visualizer."
                .to_owned(),
        );
    }
    visualizer.open()
}

#[tauri::command]
pub(crate) fn close_visualizer(visualizer: tauri::State<'_, Visualizer>) -> Result<(), String> {
    visualizer.close()
}

/// What the visualizer is doing, in words an operator can act on.
///
/// Polled here rather than pushed, so noticing a dead helper and reporting it are the same call
/// and cannot disagree.
/// What the visualizer is drawing with, once it has said. Empty before the greeting completes.
#[tauri::command]
pub(crate) fn visualizer_renderer(
    visualizer: tauri::State<'_, Visualizer>,
) -> Result<Option<String>, String> {
    visualizer.renderer()
}

#[tauri::command]
pub(crate) fn visualizer_state(visualizer: tauri::State<'_, Visualizer>) -> Result<String, String> {
    visualizer.poll()?;
    Ok(visualizer.state()?.message())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_missing_visualizer_says_how_to_get_one() {
        // The binary is looked for beside this test's executable, where it is not.
        if let Err(error) = helper_binary() {
            assert!(error.contains("npm run build:viz"), "{error}");
        }
    }

    #[test]
    fn a_visualizer_that_was_never_opened_is_down() {
        let visualizer = Visualizer::default();
        assert_eq!(visualizer.state().expect("state"), HelperState::Down);
        assert!(
            visualizer
                .state()
                .expect("state")
                .message()
                .contains("not running"),
            "an operator is told, rather than left with a blank"
        );
    }

    /// Closing something never opened is not an error: the menu item should not fail because the
    /// window an operator is closing had already gone.
    #[test]
    fn closing_a_visualizer_that_is_not_open_is_harmless() {
        let visualizer = Visualizer::default();
        visualizer.close().expect("closing is idempotent");
        visualizer.poll().expect("polling nothing is harmless");
        assert_eq!(visualizer.state().expect("state"), HelperState::Down);
    }

    /// The desk supervises a helper it ships. Anything else would be another build speaking an
    /// unknown version of the protocol.
    #[test]
    fn the_helper_is_looked_for_beside_this_application() {
        let name = renderer_binary_name();
        assert!(name.starts_with("viz-renderer"), "{name}");
        assert!(!Path::new(name).is_absolute(), "a sibling, not a path");
    }

    #[test]
    fn a_desk_renderer_is_given_the_authoritative_server_endpoint() {
        assert_eq!(
            desk_arguments("127.0.0.1:51234".parse().expect("address")),
            ["--server", "127.0.0.1", "--port", "51234"]
        );
    }
}
