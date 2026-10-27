use super::*;
use std::{fs, path::Path};

pub(super) struct Workspace {
    pub directory: PathBuf,
    pub destination: PathBuf,
}
impl Workspace {
    pub fn new(job: &Job) -> Result<Self, ImageError> {
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
            return Err(ImageError::Failed("Choose a PNG image filename".into()));
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let parent = parent.canonicalize()?;
        let snapshots = viz_snapshot::default_root();
        if parent.starts_with(snapshots.canonicalize().unwrap_or(snapshots)) {
            return Err(ImageError::Failed(
                "Save images outside the rotating scene snapshot folder".into(),
            ));
        }
        let filename = destination
            .file_name()
            .ok_or_else(|| ImageError::Failed("Choose a PNG image filename".into()))?;
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
