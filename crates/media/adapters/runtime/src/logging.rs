//! Structured logging.
//!
//! Logging is never on a real-time path: the render thread, the audio callback, and the packet
//! parsers publish into bounded channels and let a worker emit. What this module owns is only
//! where the emitted records go — the terminal, and the bounded window the log viewer reads.

use std::sync::OnceLock;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::log_buffer::LogBuffer;

/// The window the first installed subscriber filled, so a later caller gets the same one.
static WINDOW: OnceLock<LogBuffer> = OnceLock::new();

/// Installs the process log subscriber and returns the window the log viewer reads.
///
/// Safe to call more than once; only the first call installs, so a test binary that starts several
/// runtimes does not fight over the global subscriber. Every caller gets the window that is
/// actually being filled rather than an empty one of its own.
pub fn install_logging() -> LogBuffer {
    WINDOW
        .get_or_init(|| {
            let filter = EnvFilter::try_from_env("MEDIA_LOG")
                .or_else(|_| EnvFilter::try_from_default_env())
                .unwrap_or_else(|_| EnvFilter::new("info"));
            let buffer = LogBuffer::new();
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .with(buffer.layer())
                .try_init();
            buffer
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_logging_twice_returns_the_same_window() {
        let first = install_logging();
        let second = install_logging();

        tracing::info!("a record for the window");
        let query = media_http::LogQuery {
            limit: 10,
            ..Default::default()
        };
        assert_eq!(
            first.page(&query).newest,
            second.page(&query).newest,
            "two callers must not each hold half the log"
        );
    }
}
