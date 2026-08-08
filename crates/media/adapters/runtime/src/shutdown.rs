//! Structured shutdown.
//!
//! Shutdown is an ordered sequence, not a process exit: stop accepting work, cancel jobs and
//! tasks, stop decoders, flush required persistence, release GPU and window resources, join
//! threads. Every background task has an owner, a cancellation path, and somewhere to report an
//! error.

use tokio::sync::watch;

/// Why the server is stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// The operating system asked the process to stop (Ctrl-C, SIGTERM).
    Signal,
    /// Something inside the process asked for shutdown — an administrative request, or a
    /// subsystem that cannot continue.
    Requested,
}

impl ShutdownReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Signal => "signal",
            Self::Requested => "requested",
        }
    }
}

/// A shutdown broadcast every owned task can watch.
#[derive(Debug, Clone)]
pub struct Shutdown {
    sender: watch::Sender<Option<ShutdownReason>>,
}

impl Shutdown {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(None);
        Self { sender }
    }

    /// Asks every watcher to stop. The first reason wins, so a signal arriving during an
    /// administrative shutdown does not rewrite what the logs say happened.
    pub fn request(&self, reason: ShutdownReason) {
        self.sender.send_if_modified(|current| {
            if current.is_some() {
                false
            } else {
                *current = Some(reason);
                true
            }
        });
    }

    /// The reason shutdown began, if it has.
    pub fn reason(&self) -> Option<ShutdownReason> {
        *self.sender.borrow()
    }

    /// A handle an owned task waits on.
    pub fn watcher(&self) -> ShutdownWatcher {
        ShutdownWatcher {
            receiver: self.sender.subscribe(),
        }
    }

    /// Resolves once shutdown has been requested, whether by a signal or from inside the process.
    pub async fn wait_for_signal(&self) -> ShutdownReason {
        let mut watcher = self.watcher();
        tokio::select! {
            reason = watcher.wait() => reason,
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    tracing::warn!(%error, "cannot listen for the interrupt signal");
                }
                self.request(ShutdownReason::Signal);
                self.reason().unwrap_or(ShutdownReason::Signal)
            }
        }
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// One task's view of shutdown.
#[derive(Debug, Clone)]
pub struct ShutdownWatcher {
    receiver: watch::Receiver<Option<ShutdownReason>>,
}

impl ShutdownWatcher {
    /// Resolves as soon as shutdown has been requested — immediately, if it already has.
    pub async fn wait(&mut self) -> ShutdownReason {
        if let Some(reason) = *self.receiver.borrow_and_update() {
            return reason;
        }
        // The sender lives for the process's lifetime; a closed channel still means "stop".
        while self.receiver.changed().await.is_ok() {
            if let Some(reason) = *self.receiver.borrow_and_update() {
                return reason;
            }
        }
        ShutdownReason::Requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_watcher_created_before_the_request_observes_it() {
        let shutdown = Shutdown::new();
        let mut watcher = shutdown.watcher();
        shutdown.request(ShutdownReason::Requested);
        assert_eq!(watcher.wait().await, ShutdownReason::Requested);
    }

    #[tokio::test]
    async fn a_watcher_created_after_the_request_still_observes_it() {
        let shutdown = Shutdown::new();
        shutdown.request(ShutdownReason::Signal);
        assert_eq!(shutdown.watcher().wait().await, ShutdownReason::Signal);
    }

    #[test]
    fn the_first_reason_wins() {
        let shutdown = Shutdown::new();
        assert_eq!(shutdown.reason(), None);
        shutdown.request(ShutdownReason::Requested);
        shutdown.request(ShutdownReason::Signal);
        assert_eq!(shutdown.reason(), Some(ShutdownReason::Requested));
    }

    #[tokio::test]
    async fn waiting_for_a_signal_returns_an_internal_request_too() {
        let shutdown = Shutdown::new();
        let requester = shutdown.clone();
        tokio::spawn(async move { requester.request(ShutdownReason::Requested) });
        assert_eq!(shutdown.wait_for_signal().await, ShutdownReason::Requested);
    }
}
