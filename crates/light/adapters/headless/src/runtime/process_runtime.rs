//! The headless process's Tokio runtime.
//!
//! The separate thread-CPU diagnostic (`LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS=1`, only in its own
//! traced run) attributes CPU per owned thread across a fixed census window and reports partial
//! coverage when a thread exits inside it. Tokio retires a blocking thread after ten idle seconds,
//! so request handlers that ran before the window used to exit during it. That run keeps idle
//! blocking threads for the whole diagnostic; every other run keeps Tokio's default.
use std::time::Duration;

/// Idle blocking threads outlive any diagnostic run (proof window, settle and profile).
const DIAGNOSTIC_BLOCKING_KEEP_ALIVE: Duration = Duration::from_secs(60 * 60);

/// How long an idle blocking thread lives, given `LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS`; `None`
/// keeps Tokio's default.
pub(crate) fn blocking_thread_keep_alive(thread_cpu_diagnostics: Option<&str>) -> Option<Duration> {
    (thread_cpu_diagnostics == Some("1")).then_some(DIAGNOSTIC_BLOCKING_KEEP_ALIVE)
}

/// The multi-threaded runtime `#[tokio::main]` would build, with the diagnostic keep-alive.
pub fn build_tokio_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    let diagnostics = std::env::var("LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS").ok();
    if let Some(keep_alive) = blocking_thread_keep_alive(diagnostics.as_deref()) {
        builder.thread_keep_alive(keep_alive);
    }
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_thread_cpu_diagnostic_keeps_blocking_threads_through_its_window() {
        assert_eq!(
            blocking_thread_keep_alive(Some("1")),
            Some(DIAGNOSTIC_BLOCKING_KEEP_ALIVE)
        );
        // The census window plus settle and profile stay well below the keep-alive.
        assert!(DIAGNOSTIC_BLOCKING_KEEP_ALIVE >= Duration::from_secs(10 * 60));
        for other in [None, Some("0"), Some(""), Some("true"), Some("all")] {
            assert_eq!(blocking_thread_keep_alive(other), None, "{other:?}");
        }
    }
}
