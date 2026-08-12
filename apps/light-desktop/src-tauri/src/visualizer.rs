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
use viz_helper::framing::write_frame;
use viz_helper::handshake::{HelperIdentity, greet_helper};
use viz_helper::protocol::{ToHelper, encode};
use viz_helper::{HelperState, SupervisedHelper};

/// The visualizer this desk has open, if any.
///
/// One at a time: a second would be a second window drawing the same show, which is not a feature
/// anybody asked for and is two GPU contexts for one picture.
#[derive(Default)]
pub(crate) struct Visualizer {
    helper: Mutex<Option<SupervisedHelper>>,
    /// The writing end of the channel, once the helper has been greeted and accepted.
    to_helper: Mutex<Option<std::process::ChildStdin>>,
    /// What answered the greeting, for the desk's diagnostics.
    identity: Mutex<Option<HelperIdentity>>,
}

impl Visualizer {
    /// Start it, replacing one already running.
    pub(crate) fn open(&self) -> Result<(), String> {
        let program = helper_binary()?;
        // `--helper` is what makes it this desk's window rather than the standalone product: it
        // takes its scene, values and view over the channel and opens nothing of its own.
        let mut helper = SupervisedHelper::new(program, vec!["--helper".to_owned()]);
        helper.start()?;

        // Greet it before anything else. A helper this desk cannot talk to is stopped here rather
        // than left with a window showing something nobody can vouch for.
        let (mut to_helper, mut from_helper) = helper
            .take_channel()
            .ok_or("the visualizer started without a channel")?;
        let identity = match greet_helper(&mut to_helper, &mut from_helper, "ToskLight Visualizer")
        {
            Ok(identity) => identity,
            Err(error) => {
                helper.stop();
                return Err(error.to_string());
            }
        };

        *self.to_helper.lock().map_err(|_| "visualizer state")? = Some(to_helper);
        *self.identity.lock().map_err(|_| "visualizer state")? = Some(identity);
        *self.helper.lock().map_err(|_| "visualizer state")? = Some(helper);
        Ok(())
    }

    /// Send the helper a message, if one is running.
    ///
    /// A helper that has died is not an error to send to: the supervisor is already restarting or
    /// has given up, and the desk keeps running either way. The frame is dropped and the next
    /// scene the desk sends will find a channel again.
    pub(crate) fn send(&self, message: &ToHelper) -> Result<(), String> {
        let mut channel = self.to_helper.lock().map_err(|_| "visualizer state")?;
        let Some(to_helper) = channel.as_mut() else {
            return Ok(());
        };
        let payload = encode(message)?;
        if write_frame(to_helper, &payload).is_err() {
            // The pipe has gone with the process. Drop the end so the next send does not retry a
            // channel nothing is reading.
            *channel = None;
        }
        Ok(())
    }

    /// What is drawing, once the helper has said. `None` before the greeting completes.
    pub(crate) fn renderer(&self) -> Result<Option<String>, String> {
        Ok(self
            .identity
            .lock()
            .map_err(|_| "visualizer state")?
            .as_ref()
            .map(|identity| identity.renderer.clone()))
    }

    pub(crate) fn close(&self) -> Result<(), String> {
        // Ask first, so the helper closes its own window rather than being killed mid-frame.
        let _ = self.send(&ToHelper::Shutdown);
        if let Some(helper) = self.helper.lock().map_err(|_| "visualizer state")?.as_mut() {
            helper.stop();
        }
        *self.to_helper.lock().map_err(|_| "visualizer state")? = None;
        *self.identity.lock().map_err(|_| "visualizer state")? = None;
        Ok(())
    }

    /// Notice a helper that has died, and restart or give up. Driven from the desk's own loop so
    /// supervision cannot race with the desk deciding to close it.
    pub(crate) fn poll(&self) -> Result<(), String> {
        if let Some(helper) = self.helper.lock().map_err(|_| "visualizer state")?.as_mut() {
            helper.poll(std::time::Instant::now());
        }
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
    // one bundle, which is the case `npm run open` produces.
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
/// Send the visualizer the rig to draw.
///
/// The payload is already encoded by whoever built it: the desk's scene comes from the render
/// pipeline, not from this module, and re-encoding it here would be a second opinion about what
/// the helper is looking at.
#[tauri::command]
pub(crate) fn send_visualizer_scene(
    visualizer: tauri::State<'_, Visualizer>,
    payload: Vec<u8>,
) -> Result<(), String> {
    visualizer.send(&ToHelper::Scene { payload })
}

/// Send the visualizer what the rig is currently doing.
#[tauri::command]
pub(crate) fn send_visualizer_values(
    visualizer: tauri::State<'_, Visualizer>,
    payload: Vec<u8>,
) -> Result<(), String> {
    visualizer.send(&ToHelper::Values { payload })
}

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

    /// Sending to a visualizer that is not running is not an error. The supervisor is already
    /// restarting it or has given up, and the desk carries on either way — a show does not stop
    /// because a window went.
    #[test]
    fn sending_to_a_visualizer_that_is_not_running_is_harmless() {
        let visualizer = Visualizer::default();
        visualizer
            .send(&ToHelper::Shutdown)
            .expect("sending into the void is not a failure");
        assert_eq!(visualizer.renderer().expect("renderer"), None);
    }

    /// The desk supervises a helper it ships. Anything else would be another build speaking an
    /// unknown version of the protocol.
    #[test]
    fn the_helper_is_looked_for_beside_this_application() {
        let name = renderer_binary_name();
        assert!(name.starts_with("viz-renderer"), "{name}");
        assert!(!Path::new(name).is_absolute(), "a sibling, not a path");
    }
}
