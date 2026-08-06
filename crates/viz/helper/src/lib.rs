#![forbid(unsafe_code)]
//! Supervising a renderer helper process.
//!
//! The desk must be able to open a visualizer without taking on its risk. A renderer draws with a
//! GPU driver, and a GPU driver can take a process down in ways no amount of care in our own code
//! prevents — so the desk runs it as a child rather than a thread. A helper that dies takes its
//! window with it and reports the failure; the desk's Programmer, playback and output engine carry
//! on, because they were never in that address space.
//!
//! This is the supervision alone: starting, noticing death, restarting with a bounded backoff, and
//! cleaning up whatever happens. What the helper renders and how it is told about it belong to the
//! protocol on top, and are deliberately not here — supervision that can only be exercised through
//! a GPU is supervision nobody tests.

pub mod channel;
pub mod framing;
pub mod handshake;
pub mod protocol;

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait after a crash before starting the helper again, and the ceiling that backs
/// off to.
///
/// A helper that dies immediately and repeatedly is not going to succeed on the next attempt
/// either, so the gap grows. It stops growing at the ceiling because an operator who fixes the
/// cause — plugs the display back in, updates the driver — should not then wait minutes.
const FIRST_RETRY: Duration = Duration::from_millis(250);
const RETRY_CEILING: Duration = Duration::from_secs(5);

/// How many times a helper may die before it is left down.
///
/// Past this the fault is not transient and restarting forever would hide it behind a window that
/// flickers back into existence. The operator is told, and asks for it again when they are ready.
const GIVE_UP_AFTER: u32 = 5;

/// What the supervisor is currently doing, for the surface that has to say so.
#[derive(Clone, Debug, PartialEq)]
pub enum HelperState {
    /// Never started, or deliberately stopped.
    Down,
    Running,
    /// Died, and will start again once the gap has passed.
    Restarting {
        failures: u32,
        detail: String,
    },
    /// Died too often to keep trying. Only an explicit start clears this.
    GaveUp {
        failures: u32,
        detail: String,
    },
}

impl HelperState {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// What an operator is told. Every state says something rather than leaving a blank window
    /// with no explanation.
    pub fn message(&self) -> String {
        match self {
            Self::Down => "The visualizer is not running.".to_owned(),
            Self::Running => "The visualizer is running.".to_owned(),
            Self::Restarting { failures, detail } => {
                format!("The visualizer stopped ({detail}); restarting, attempt {failures}.")
            }
            Self::GaveUp { failures, detail } => format!(
                "The visualizer stopped {failures} times and will not be restarted again \
                 ({detail}). The desk is unaffected; open it again when the cause is fixed."
            ),
        }
    }
}

/// A renderer helper and the supervision around it.
///
/// Dropping this ends the child. A helper outliving the desk that owns it would leave a window
/// nothing can close and a GPU context nothing will release.
pub struct SupervisedHelper {
    program: std::path::PathBuf,
    arguments: Vec<String>,
    child: Option<Child>,
    state: HelperState,
    failures: u32,
    restart_at: Option<Instant>,
}

impl SupervisedHelper {
    /// Describe a helper without starting it. Nothing is spawned until [`Self::start`].
    pub fn new(program: impl Into<std::path::PathBuf>, arguments: Vec<String>) -> Self {
        Self {
            program: program.into(),
            arguments,
            child: None,
            state: HelperState::Down,
            failures: 0,
            restart_at: None,
        }
    }

    pub fn state(&self) -> &HelperState {
        &self.state
    }

    /// The channel to the running helper, taken once.
    ///
    /// Taken rather than borrowed because the two ends go to whatever drives them — typically a
    /// reader thread and the desk's own loop — and two writers on one pipe would interleave halves
    /// of different frames.
    pub fn take_channel(
        &mut self,
    ) -> Option<(std::process::ChildStdin, std::process::ChildStdout)> {
        let child = self.child.as_mut()?;
        match (child.stdin.take(), child.stdout.take()) {
            (Some(to_helper), Some(from_helper)) => Some((to_helper, from_helper)),
            // Already taken, or the child was started without pipes. Either way there is no
            // channel to hand out a second time.
            _ => None,
        }
    }

    /// Start the helper, clearing any history of it having failed.
    ///
    /// This is the operator asking for it, so it is also how a helper that gave up is given
    /// another chance.
    pub fn start(&mut self) -> Result<(), String> {
        self.stop();
        self.failures = 0;
        self.restart_at = None;
        self.spawn()
    }

    fn spawn(&mut self) -> Result<(), String> {
        // Stdin and stdout are the private channel: the desk writes scene, values and view down
        // one and reads the helper's answers up the other. Stderr is discarded rather than
        // inherited — a child writing over the desk's console would be the desk's problem, which
        // is the opposite of isolation.
        let child = Command::new(&self.program)
            .args(&self.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", self.program.display()))?;
        self.child = Some(child);
        self.state = HelperState::Running;
        Ok(())
    }

    /// Notice what has happened to the helper and act on it.
    ///
    /// Called from the desk's own loop rather than from a thread of its own, so supervision cannot
    /// race with the desk deciding to stop.
    pub fn poll(&mut self, now: Instant) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                // Still running.
                Ok(None) => return,
                Ok(Some(status)) => {
                    let detail = match status.code() {
                        Some(code) => format!("exit status {code}"),
                        None => "killed by a signal".to_owned(),
                    };
                    self.died(detail, now);
                }
                Err(error) => self.died(error.to_string(), now),
            }
            return;
        }
        // Down, waiting to try again.
        if let Some(due) = self.restart_at
            && now >= due
        {
            self.restart_at = None;
            if let Err(error) = self.spawn() {
                self.died(error, now);
            }
        }
    }

    fn died(&mut self, detail: String, now: Instant) {
        self.child = None;
        self.failures += 1;
        if self.failures >= GIVE_UP_AFTER {
            self.state = HelperState::GaveUp {
                failures: self.failures,
                detail,
            };
            self.restart_at = None;
            return;
        }
        self.state = HelperState::Restarting {
            failures: self.failures,
            detail,
        };
        self.restart_at = Some(now + backoff(self.failures));
    }

    /// End the helper deliberately. Idempotent: stopping something already down is not an error.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.state = HelperState::Down;
        self.restart_at = None;
    }
}

impl Drop for SupervisedHelper {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The gap before the `failures`-th restart, doubling and then holding at the ceiling.
fn backoff(failures: u32) -> Duration {
    let doubled = FIRST_RETRY * 2_u32.saturating_pow(failures.saturating_sub(1).min(16));
    doubled.min(RETRY_CEILING)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A helper that stays up, and one that exits immediately, without needing a GPU for either.
    fn sleeper() -> SupervisedHelper {
        SupervisedHelper::new("/bin/sleep", vec!["30".to_owned()])
    }

    fn quitter() -> SupervisedHelper {
        SupervisedHelper::new("/usr/bin/false", Vec::new())
    }

    #[test]
    fn a_helper_that_will_not_start_is_reported_rather_than_pretended() {
        let mut helper = SupervisedHelper::new("/definitely/not/here", Vec::new());
        let error = helper.start().expect_err("a missing program cannot start");
        assert!(error.contains("/definitely/not/here"), "{error}");
        assert_eq!(helper.state(), &HelperState::Down);
    }

    #[test]
    fn a_running_helper_stays_running() {
        let mut helper = sleeper();
        helper.start().expect("sleep starts");
        helper.poll(Instant::now());
        assert!(helper.state().is_running());
    }

    #[test]
    fn a_helper_that_dies_is_noticed_and_scheduled_to_return() {
        let mut helper = quitter();
        helper.start().expect("false starts");
        // Give it a moment to exit, then notice.
        std::thread::sleep(Duration::from_millis(50));
        helper.poll(Instant::now());
        match helper.state() {
            HelperState::Restarting { failures, .. } => assert_eq!(*failures, 1),
            other => panic!("expected a restart, got {other:?}"),
        }
    }

    /// A helper that cannot succeed must not be restarted forever: the operator is told once and
    /// the window stops flickering back.
    #[test]
    fn a_helper_that_keeps_dying_is_eventually_left_down() {
        let mut helper = quitter();
        helper.start().expect("false starts");
        let mut now = Instant::now();
        for _ in 0..GIVE_UP_AFTER * 2 {
            std::thread::sleep(Duration::from_millis(20));
            // Advance past whatever backoff was set, so the supervisor keeps trying.
            now += RETRY_CEILING * 2;
            helper.poll(now);
        }
        match helper.state() {
            HelperState::GaveUp { failures, .. } => assert!(*failures >= GIVE_UP_AFTER),
            other => panic!("expected it to give up, got {other:?}"),
        }
        assert!(
            helper.state().message().contains("desk is unaffected"),
            "the operator is told the desk is fine: {}",
            helper.state().message()
        );
    }

    /// Asking for it again is how a helper that gave up comes back.
    #[test]
    fn starting_again_clears_a_helper_that_gave_up() {
        let mut helper = quitter();
        helper.start().expect("starts");
        let mut now = Instant::now();
        for _ in 0..GIVE_UP_AFTER * 2 {
            std::thread::sleep(Duration::from_millis(20));
            now += RETRY_CEILING * 2;
            helper.poll(now);
        }
        assert!(matches!(helper.state(), HelperState::GaveUp { .. }));

        let mut helper = sleeper();
        helper.start().expect("a fresh start");
        assert!(helper.state().is_running());
    }

    /// The channel is the whole reason the helper is a child rather than a thread, so a frame has
    /// to survive the round trip through it. `cat` echoes whatever it is given.
    #[test]
    fn a_frame_survives_the_channel_to_a_running_helper() {
        let mut helper = SupervisedHelper::new("/bin/cat", Vec::new());
        helper.start().expect("cat starts");
        let (mut to_helper, mut from_helper) = helper
            .take_channel()
            .expect("a channel to a running helper");

        crate::framing::write_frame(&mut to_helper, b"a scene").expect("writes");
        let echoed = crate::framing::read_frame(&mut from_helper).expect("reads");
        assert_eq!(echoed, b"a scene");
    }

    #[test]
    fn the_channel_is_handed_out_once() {
        let mut helper = SupervisedHelper::new("/bin/cat", Vec::new());
        helper.start().expect("cat starts");
        assert!(helper.take_channel().is_some());
        assert!(
            helper.take_channel().is_none(),
            "two writers on one pipe would interleave halves of different frames"
        );
    }

    #[test]
    fn a_helper_that_is_not_running_has_no_channel() {
        let mut helper = sleeper();
        assert!(helper.take_channel().is_none());
    }

    #[test]
    fn stopping_is_idempotent() {
        let mut helper = sleeper();
        helper.start().expect("starts");
        helper.stop();
        helper.stop();
        assert_eq!(helper.state(), &HelperState::Down);
    }

    /// The backoff grows and then holds, so a fixed cause is not punished by a long wait.
    #[test]
    fn the_backoff_grows_and_then_holds() {
        assert_eq!(backoff(1), FIRST_RETRY);
        assert!(backoff(2) > backoff(1));
        assert_eq!(backoff(20), RETRY_CEILING);
    }

    /// Every state says something an operator can act on.
    #[test]
    fn every_state_explains_itself() {
        for state in [
            HelperState::Down,
            HelperState::Running,
            HelperState::Restarting {
                failures: 1,
                detail: "exit status 1".to_owned(),
            },
            HelperState::GaveUp {
                failures: 5,
                detail: "exit status 1".to_owned(),
            },
        ] {
            assert!(!state.message().is_empty(), "{state:?} says nothing");
        }
    }
}
