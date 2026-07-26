use super::*;

pub(super) fn sibling_fixture_package_dir(executable: &FsPath) -> Option<PathBuf> {
    let directory = executable.parent()?.join("fixture-library");
    directory.is_dir().then_some(directory)
}

pub(super) async fn run_server() -> anyhow::Result<()> {
    bootstrap::run().await
}

pub(super) fn router(state: AppState) -> Router {
    http_router::build(state)
}
