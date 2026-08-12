//! The show this window had open last time.
//!
//! A visualizer that opened this window did so because it had nothing to draw. Reopening the last
//! rig means the picture is there immediately, which is what an operator coming back to a plan
//! expects. A file that has since been moved or deleted is forgotten rather than reported as a
//! failure — it is not an error to have finished with a show.

use std::path::{Path, PathBuf};

/// Remembers one path, in the operator's own configuration directory rather than anywhere the
/// repository owns.
pub struct RecentShow {
    file: PathBuf,
    legacy_file: Option<PathBuf>,
}

impl RecentShow {
    pub fn at(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            legacy_file: None,
        }
    }

    pub fn migrating_from(file: impl Into<PathBuf>, legacy_file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            legacy_file: Some(legacy_file.into()),
        }
    }

    /// The last show, if it is still there.
    pub fn read(&self) -> Option<PathBuf> {
        let recorded = std::fs::read_to_string(&self.file).ok().or_else(|| {
            self.legacy_file
                .as_ref()
                .and_then(|legacy| std::fs::read_to_string(legacy).ok())
        })?;
        let path = PathBuf::from(recorded.trim());
        if path.is_file() {
            self.remember(&path);
            Some(path)
        } else {
            None
        }
    }

    pub fn remember(&self, path: &Path) {
        if let Some(parent) = self.file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&self.file, path.display().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn scratch() -> PathBuf {
        let base = std::env::var_os("LIGHT_TMP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join(format!("viz-editor-recent-{}", Uuid::new_v4()))
    }

    #[test]
    fn the_last_show_is_offered_again() {
        let directory = scratch();
        std::fs::create_dir_all(&directory).unwrap();
        let show = directory.join("tour.show");
        std::fs::write(&show, "not really a show, but a file").unwrap();
        let recent = RecentShow::at(directory.join("recent"));

        assert_eq!(recent.read(), None, "nothing has been opened yet");
        recent.remember(&show);
        assert_eq!(recent.read(), Some(show.clone()));

        std::fs::remove_file(&show).unwrap();
        assert_eq!(
            recent.read(),
            None,
            "a show that has been moved away is forgotten, not reported as broken"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_legacy_editor_recent_show_moves_to_the_visualizer_identity() {
        let directory = scratch();
        let show = directory.join("tour.show");
        let legacy = directory.join("legacy/recent-show");
        let current = directory.join("visualizer/recent-show");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&show, "show").unwrap();
        std::fs::write(&legacy, show.display().to_string()).unwrap();

        let recent = RecentShow::migrating_from(&current, legacy);
        assert_eq!(recent.read(), Some(show));
        assert!(
            current.is_file(),
            "the new identity owns the migrated record"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
