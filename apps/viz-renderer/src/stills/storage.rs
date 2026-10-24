use super::*;
use std::{fs, path::Path};
use viz_snapshot::ultimate::ImageManifest;

pub(super) struct Workspace {
    pub directory: PathBuf,
    pub destination: PathBuf,
}
impl Workspace {
    pub fn new(job: &Job) -> Result<Self, UltimateError> {
        let destination = job.options.destination.clone().unwrap_or_else(|| {
            let directory = std::env::var_os("TOSKLIGHT_VIZ_IMAGE_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| viz_snapshot::default_root().with_file_name("Images"));
            let name = job
                .frozen
                .scene()
                .show_name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .take(60)
                .collect::<String>();
            directory.join(format!(
                "{}-{}.png",
                if name.is_empty() { "Stage" } else { &name },
                job.id.0
            ))
        });
        if !destination
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            return Err(UltimateError::Failed("Choose a PNG image filename".into()));
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let parent = parent.canonicalize()?;
        let snapshots = viz_snapshot::default_root();
        if parent.starts_with(snapshots.canonicalize().unwrap_or(snapshots)) {
            return Err(UltimateError::Failed(
                "Save images outside the rotating scene snapshot folder".into(),
            ));
        }
        let filename = destination
            .file_name()
            .ok_or_else(|| UltimateError::Failed("Choose a PNG image filename".into()))?;
        let destination = parent.join(filename);
        let directory = parent.join(format!(".tosklight-still-{}", job.id.0));
        fs::create_dir(&directory)?; // Never reuse or remove a directory we did not create.
        Ok(Self {
            directory,
            destination,
        })
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

pub(super) fn package(
    job: &Job,
    workspace: &Workspace,
    manifest: &ImageManifest,
) -> Result<PathBuf, UltimateError> {
    job.check_cancel()?;
    let frozen = &job.frozen;
    let directory = workspace.directory.join("frozen");
    fs::create_dir(&directory)?;
    manifest.write_package_with_cancel(&directory, &job.cancel)?;
    let media: Vec<_> = frozen
        .media()
        .sources()
        .iter()
        .map(|source| {
            serde_json::json!({
                "sourceId": source.frame.source_id, "sequence": source.frame.sequence,
                "available": source.available, "flicker": source.flicker,
            })
        })
        .collect();
    let metadata = serde_json::json!({ "jobId": job.id, "paneId": frozen.origin().pane_id,
        "followPreload": frozen.origin().follow_preload, "sceneRevision": frozen.scene().revision,
        "valuesFrame": frozen.values().frame, "timeSeconds": frozen.time_seconds(), "view": frozen.view(), "media": media });
    fs::write(
        directory.join("frozen-state.json"),
        serde_json::to_vec_pretty(&metadata)
            .map_err(|error| UltimateError::Failed(error.to_string()))?,
    )?;
    Ok(directory)
}
