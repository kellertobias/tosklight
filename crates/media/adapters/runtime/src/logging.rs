//! Structured logging.
//!
//! Logging is never on a real-time path: the render thread, the audio callback, and the packet
//! parsers publish into bounded channels and let a worker emit. What this module owns is only
//! where the emitted records go — the terminal, and the bounded window the log viewer reads.

use std::sync::{Arc, OnceLock, RwLock};

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::reload;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::log_buffer::LogBuffer;

type UpdateLogLevel = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// The subscriber installed for this process.
static LOGGING: OnceLock<InstalledLogging> = OnceLock::new();

/// The bounded log window and the live filter control owned by the installed subscriber.
#[derive(Clone)]
pub struct InstalledLogging {
    pub window: LogBuffer,
    current: Arc<RwLock<String>>,
    update: UpdateLogLevel,
}

impl InstalledLogging {
    pub fn control(&self) -> media_http::LogLevelControl {
        let reading = Arc::clone(&self.current);
        let update = Arc::clone(&self.update);
        media_http::LogLevelControl {
            current: Arc::new(move || {
                reading
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
            }),
            update,
        }
    }
}

/// Installs the process log subscriber and returns the window the log viewer reads.
///
/// Safe to call more than once; only the first call installs, so a test binary that starts several
/// runtimes does not fight over the global subscriber. Every caller gets the window that is
/// actually being filled rather than an empty one of its own.
pub fn install_logging() -> InstalledLogging {
    LOGGING
        .get_or_init(|| {
            let filter = EnvFilter::try_from_env("MEDIA_LOG")
                .or_else(|_| EnvFilter::try_from_default_env())
                .unwrap_or_else(|_| EnvFilter::new("info"));
            let current = Arc::new(RwLock::new(filter.to_string()));
            let (filter, reload) = reload::Layer::new(filter);
            let buffer = LogBuffer::new();
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_target(false))
                .with(buffer.layer())
                .try_init();
            let updating = Arc::clone(&current);
            InstalledLogging {
                window: buffer,
                current,
                update: Arc::new(move |level| {
                    reload
                        .reload(EnvFilter::new(level))
                        .map_err(|error| error.to_string())?;
                    *updating
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = level.to_owned();
                    Ok(())
                }),
            }
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
            first.window.page(&query).newest,
            second.window.page(&query).newest,
            "two callers must not each hold half the log"
        );
    }
}
