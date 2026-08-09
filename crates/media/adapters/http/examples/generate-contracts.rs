use std::path::Path;

fn main() -> std::io::Result<()> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    media_http::write_generated_artifacts(&workspace_root)
}
