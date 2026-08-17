//! `--verify`: prove the window actually drew its interface, then exit.
//!
//! The editor once opened white on every locally built binary, for a whole class of reason no
//! build catches: Tauri embeds no frontend without the `custom-protocol` feature and falls back to
//! the dev-server URL, so the application starts, the window opens, and nothing is in it. A build
//! that compiles is not evidence of a window that drew.
//!
//! So the frontend reports when its document surface has mounted, and this waits for that report.
//! The check is the round trip itself: the binary embedded a frontend, the webview loaded it, the
//! bundle executed, React mounted, and the command channel works. Any of those failing is the
//! white window, and any of them failing fails this.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// How long the interface gets to mount before this is called a failure.
///
/// Generous on purpose: a cold start on a loaded build machine is slow, and a flaky check that
/// fails a good build teaches people to ignore it. Windows CI creates the WebView2 user data
/// folder on first launch behind a virus scanner, which is minutes slower than a warm desktop.
const MOUNT_TIMEOUT: Duration = Duration::from_secs(120);

/// Whether this launch is only checking that the window draws.
pub fn requested() -> bool {
    std::env::args()
        .skip(1)
        .any(|argument| argument == "--verify")
}

/// The frontend's report that it mounted, waited on by the verifying thread.
#[derive(Default)]
pub struct SurfaceReady {
    mounted: Mutex<bool>,
    changed: Condvar,
    /// Set once the outcome has been reported, so a second mount cannot exit twice.
    settled: AtomicBool,
}

impl SurfaceReady {
    pub fn mark_mounted(&self) {
        *self.mounted.lock().expect("surface state") = true;
        self.changed.notify_all();
    }

    /// Wait for the interface, and answer whether it arrived in time.
    fn wait(&self) -> bool {
        let mut mounted = self.mounted.lock().expect("surface state");
        let deadline = std::time::Instant::now() + MOUNT_TIMEOUT;
        while !*mounted {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (guard, _) = self
                .changed
                .wait_timeout(mounted, remaining)
                .expect("surface state");
            mounted = guard;
        }
        true
    }

    fn settle(&self) -> bool {
        !self.settled.swap(true, Ordering::SeqCst)
    }
}

/// The command the frontend calls once its document surface is on screen.
///
/// Always registered, not only under `--verify`: a command that exists solely in a checking mode
/// is a command nobody proves works, and the frontend would have to know which mode it is in.
#[tauri::command]
pub fn surface_ready(ready: tauri::State<'_, Arc<SurfaceReady>>) {
    ready.mark_mounted();
}

/// Watch for the interface and end the process with the verdict.
///
/// Exits rather than returning, because there is nothing else this launch was for. The message
/// says which of the two happened in the words someone reading CI output needs.
pub fn watch(ready: Arc<SurfaceReady>, loaded: impl Fn() -> String + Send + 'static) {
    std::thread::Builder::new()
        .name("viz-editor-verify".into())
        .spawn(move || {
            let mounted = ready.wait();
            if !ready.settle() {
                return;
            }
            if mounted {
                println!("The Viz editor drew its interface.");
                std::process::exit(0);
            }
            eprintln!(
                "The Viz editor opened but drew nothing within {} seconds. The window is white: \
                 either the binary embedded no frontend — a build without the `custom-protocol` \
                 feature falls back to the dev-server URL — or the interface failed to mount. \
                 The window is showing {}.",
                MOUNT_TIMEOUT.as_secs(),
                loaded()
            );
            std::process::exit(1);
        })
        .expect("verify thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mounted_surface_is_seen() {
        let ready = Arc::new(SurfaceReady::default());
        ready.mark_mounted();
        assert!(
            ready.wait(),
            "a surface that mounted before the wait counts"
        );
    }

    #[test]
    fn a_surface_that_mounts_while_waiting_is_seen() {
        let ready = Arc::new(SurfaceReady::default());
        let reporter = ready.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            reporter.mark_mounted();
        });
        assert!(ready.wait());
    }

    /// The verdict is reported once however many times the frontend says it mounted, so a reload
    /// during the check cannot exit the process twice with different answers.
    #[test]
    fn the_outcome_settles_only_once() {
        let ready = SurfaceReady::default();
        assert!(ready.settle());
        assert!(!ready.settle());
    }
}
