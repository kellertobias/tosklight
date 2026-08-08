//! Structured logging.
//!
//! Logging is never on a real-time path: the render thread, the audio callback, and the packet
//! parsers publish into bounded channels and let a worker emit. What this module owns is only
//! where the emitted records go.

use std::sync::Once;

use tracing_subscriber::EnvFilter;

static INSTALLED: Once = Once::new();

/// Installs the process log subscriber. Safe to call more than once; only the first call wins,
/// so a test binary that starts several runtimes does not fight over the global subscriber.
pub fn install_logging() {
    INSTALLED.call_once(|| {
        let filter = EnvFilter::try_from_env("MEDIA_LOG")
            .or_else(|_| EnvFilter::try_from_default_env())
            .unwrap_or_else(|_| EnvFilter::new("info"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .try_init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_logging_twice_is_harmless() {
        install_logging();
        install_logging();
    }
}
