#![forbid(unsafe_code)]

// Thin process entry point. The executable owns no Media behavior; it delegates startup to the
// runtime adapter so lifecycle composition stays separate from domain and application code.
//
// It is not `#[tokio::main]`: a windowed output needs the platform event loop on this thread, so
// the runtime adapter owns both the loop and the asynchronous runtime the services run on.
fn main() -> anyhow::Result<()> {
    media_runtime::run()
}
