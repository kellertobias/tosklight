//! Taking snapshots, keeping them, and exporting them to Blender.
//!
//! Pressing the key must not cost a frame, and exporting must not stop the picture: a rig of three
//! hundred fixtures is megabytes of geometry to write, and Blender takes seconds to open one. So
//! the only thing that happens on the thread that draws is the freeze itself — a copy of the scene
//! and its values, which is the moment the operator asked for — and everything after that happens
//! on a worker whose results are collected the next time a frame is built.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use viz_scene::{Scene, SceneValues};
use viz_snapshot::{
    CaptureContext, ExportError, SnapshotEntry, SnapshotStore, capture, export_blend,
};

/// How long a confirmation stays on the status surface. Long enough to read from arm's length,
/// short enough that it is gone before it is in the way of the picture.
const NOTICE_SECONDS: f32 = 5.0;

/// Where one capture's Blender export has got to.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ExportState {
    #[default]
    Idle,
    Running,
    Done(PathBuf),
    Failed(String),
}

/// One kept capture and what has been done with it.
#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub entry: SnapshotEntry,
    pub export: ExportState,
}

impl SnapshotRow {
    /// The value shown beside the row: what the export is doing, or what is in the capture.
    pub fn status(&self) -> String {
        match &self.export {
            ExportState::Idle => match &self.entry.blend {
                Some(_) => format!("{} \u{2014} exported", self.entry.summary()),
                None => format!("{} \u{2014} Enter exports", self.entry.summary()),
            },
            ExportState::Running => "Exporting to Blender\u{2026}".to_owned(),
            ExportState::Done(path) => match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => format!("Exported {name}"),
                None => "Exported".to_owned(),
            },
            ExportState::Failed(reason) => reason.clone(),
        }
    }

    /// Whether the row is reporting something that went wrong, so it can be coloured as such.
    pub fn is_failure(&self) -> bool {
        matches!(self.export, ExportState::Failed(_))
    }
}

/// What a worker finished doing.
enum Finished {
    Captured(Result<SnapshotEntry, String>),
    Exported {
        directory: PathBuf,
        outcome: Result<PathBuf, String>,
    },
}

pub struct Snapshots {
    store: SnapshotStore,
    rows: Vec<SnapshotRow>,
    sender: Sender<Finished>,
    events: Receiver<Finished>,
    notice: Option<(String, Instant, bool)>,
    /// Workers still running. A scripted run waits on this; the interactive application never
    /// looks at it, because it simply carries on drawing.
    outstanding: usize,
    /// The last capture that was written, for a scripted run to report.
    last_written: Option<PathBuf>,
}

impl Default for Snapshots {
    fn default() -> Self {
        Self::new(SnapshotStore::default())
    }
}

impl Snapshots {
    pub fn new(store: SnapshotStore) -> Self {
        let (sender, events) = channel();
        let rows = store
            .list()
            .into_iter()
            .map(|entry| SnapshotRow {
                entry,
                export: ExportState::Idle,
            })
            .collect();
        Self {
            store,
            rows,
            sender,
            events,
            notice: None,
            outstanding: 0,
            last_written: None,
        }
    }

    pub fn rows(&self) -> &[SnapshotRow] {
        &self.rows
    }

    pub fn folder(&self) -> &std::path::Path {
        self.store.root()
    }

    /// Freeze this moment and write it in the background.
    ///
    /// The copy is what makes the capture instant and honest: everything after this point works on
    /// the rig as it was when the key went down, however long the writing takes and whatever the
    /// desk does next.
    pub fn take(&mut self, scene: &Scene, values: &SceneValues, context: CaptureContext) {
        let scene = scene.clone();
        let values = values.clone();
        let store = self.store.clone();
        let sender = self.sender.clone();
        self.announce("Snapshot taken", false);
        self.outstanding += 1;
        std::thread::spawn(move || {
            let captured = capture(&scene, &values, &context);
            let outcome = store
                .write(&captured)
                .map_err(|error| format!("Snapshot could not be written: {error}"));
            let _ = sender.send(Finished::Captured(outcome));
        });
    }

    /// Export one kept capture to a Blender file.
    ///
    /// `blender` is whatever the operator configured; empty means find one.
    pub fn export(&mut self, index: usize, blender: Option<PathBuf>) {
        let Some(row) = self.rows.get_mut(index) else {
            return;
        };
        if row.export == ExportState::Running {
            return;
        }
        row.export = ExportState::Running;
        let directory = row.entry.directory.clone();
        let destination = row.entry.blend_destination();
        let sender = self.sender.clone();
        self.announce("Exporting to Blender\u{2026}", false);
        self.outstanding += 1;
        std::thread::spawn(move || {
            let outcome = export_blend(&directory, &destination, blender.as_deref())
                .map(|exported| exported.path)
                .map_err(|error: ExportError| error.to_string());
            let _ = sender.send(Finished::Exported { directory, outcome });
        });
    }

    /// Fold in whatever the workers finished since the last frame.
    pub fn pump(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.outstanding = self.outstanding.saturating_sub(1);
            match event {
                Finished::Captured(Ok(entry)) => {
                    let notice = format!(
                        "Snapshot {} saved \u{2014} {}",
                        entry.label(),
                        entry.summary()
                    );
                    self.last_written = Some(entry.directory.clone());
                    // The store decides how many are kept, so the list is re-read from it rather
                    // than grown here and left holding rows whose folders have just been removed.
                    self.reload();
                    self.announce(&notice, false);
                }
                Finished::Captured(Err(reason)) => self.announce(&reason, true),
                Finished::Exported { directory, outcome } => {
                    let notice = match &outcome {
                        Ok(path) => format!(
                            "Exported {}",
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("Blender file")
                        ),
                        Err(reason) => reason.clone(),
                    };
                    let failed = outcome.is_err();
                    if let Some(row) = self
                        .rows
                        .iter_mut()
                        .find(|row| row.entry.directory == directory)
                    {
                        row.export = match outcome {
                            Ok(path) => {
                                row.entry.blend = Some(path.clone());
                                ExportState::Done(path)
                            }
                            Err(reason) => ExportState::Failed(reason),
                        };
                    }
                    self.announce(&notice, failed);
                }
            }
        }
    }

    /// Re-read the folder, keeping whatever each row was reporting.
    fn reload(&mut self) {
        let previous: Vec<(PathBuf, ExportState)> = self
            .rows
            .iter()
            .map(|row| (row.entry.directory.clone(), row.export.clone()))
            .collect();
        self.rows = self
            .store
            .list()
            .into_iter()
            .map(|entry| {
                let export = previous
                    .iter()
                    .find(|(directory, _)| *directory == entry.directory)
                    .map(|(_, state)| state.clone())
                    .unwrap_or_default();
                SnapshotRow { entry, export }
            })
            .collect();
    }

    /// Whether a capture or an export is still being written.
    pub fn busy(&self) -> bool {
        self.outstanding > 0
    }

    /// The folder the last capture went into.
    pub fn last_written(&self) -> Option<&std::path::Path> {
        self.last_written.as_deref()
    }

    /// Say something on the status surface without any work behind it.
    pub fn report(&mut self, message: &str) {
        self.announce(message, false);
    }

    fn announce(&mut self, message: &str, failure: bool) {
        self.notice = Some((message.to_owned(), Instant::now(), failure));
    }

    /// The confirmation to draw on the status surface, and whether it is bad news.
    pub fn notice(&self) -> Option<(&str, bool)> {
        let (message, since, failure) = self.notice.as_ref()?;
        // A failure stays until something else happens: it is the only place the operator will
        // be told, and it names something they have to go and do.
        if !failure && since.elapsed() > Duration::from_secs_f32(NOTICE_SECONDS) {
            return None;
        }
        Some((message.as_str(), *failure))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_snapshot::SnapshotCounts;

    fn entry(time: &str) -> SnapshotEntry {
        SnapshotEntry {
            directory: PathBuf::from(format!("/tmp/{time}")),
            captured_at: format!("2026-07-31 {time}"),
            show: "Tour".into(),
            counts: SnapshotCounts {
                fixtures: 12,
                heads: 12,
                live_beams: 4,
                triangles: 900,
            },
            blend: None,
        }
    }

    #[test]
    fn a_capture_that_has_not_been_exported_says_how_to_export_it() {
        let row = SnapshotRow {
            entry: entry("14:22:08"),
            export: ExportState::Idle,
        };
        assert_eq!(row.status(), "12 fixtures, 4 live \u{2014} Enter exports");
        assert!(!row.is_failure());
    }

    #[test]
    fn a_running_export_says_so_rather_than_looking_idle() {
        let row = SnapshotRow {
            entry: entry("14:22:08"),
            export: ExportState::Running,
        };
        assert_eq!(row.status(), "Exporting to Blender\u{2026}");
    }

    #[test]
    fn a_failed_export_names_what_went_wrong_and_stays_visible() {
        let mut snapshots = Snapshots::new(SnapshotStore::new(
            std::env::temp_dir().join("viz-snapshot-notice"),
            4,
        ));
        snapshots.announce("Blender not found", true);
        assert_eq!(snapshots.notice(), Some(("Blender not found", true)));

        let row = SnapshotRow {
            entry: entry("14:22:08"),
            export: ExportState::Failed("Blender not found".into()),
        };
        assert!(row.is_failure());
        assert_eq!(row.status(), "Blender not found");
    }

    #[test]
    fn an_exported_capture_names_the_file_it_produced() {
        let row = SnapshotRow {
            entry: entry("14:22:08"),
            export: ExportState::Done(PathBuf::from("/tmp/x/rig.blend")),
        };
        assert_eq!(row.status(), "Exported rig.blend");
    }
}
