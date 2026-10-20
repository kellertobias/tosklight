#![forbid(unsafe_code)]

// @tour orientation:10 Thin process entry point
// The executable owns no desk behavior. It delegates startup to the headless runtime, keeping
// lifecycle composition separate from domain and application code.
fn main() -> anyhow::Result<()> {
    light_headless_runtime::build_tokio_runtime()?
        .block_on(async { light_headless_runtime::run().await })
}
