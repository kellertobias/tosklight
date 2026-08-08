#![forbid(unsafe_code)]

// Thin process entry point. The executable owns no Media behavior; it delegates startup to the
// runtime adapter so lifecycle composition stays separate from domain and application code.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    media_runtime::run().await
}
