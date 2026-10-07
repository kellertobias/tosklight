//! Preserve desk runtime before allowing recovery to create new live state. Unlike a show-file
//! migration these records live in desk.sqlite and may be replaced on the next surface login.
use std::{fs::OpenOptions, io::Write, path::Path};

pub(super) fn preserve(
    data_dir: &Path,
    kind: &str,
    identity: uuid::Uuid,
    serialized: &str,
    error: &anyhow::Error,
) -> anyhow::Result<String> {
    let directory = data_dir.join("backups");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("runtime-recovery-{}.json", uuid::Uuid::new_v4()));
    let report = serde_json::json!({
        "kind": kind,
        "identity": identity,
        "recorded_at": chrono::Utc::now().to_rfc3339(),
        "reason": error.to_string(),
        // Exact original text, including malformed JSON. Never include authentication tokens.
        "serialized": serialized,
    });
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    let message = format!(
        "The stored {kind} could not be restored: {error}. The original data is preserved at {}. Open a compatible show or create a separate empty show to continue.",
        path.display()
    );
    tracing::error!(%identity, report=%path.display(), %error, "starting in desk runtime recovery mode");
    Ok(message)
}
