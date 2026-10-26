//! Freeze scene snapshots and retain their JSON and GLB files without interrupting rendering.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use viz_scene::{Scene, SceneValues};
use viz_snapshot::{CaptureContext, SnapshotEntry, SnapshotStore, capture};

/// How long a confirmation stays on the status surface. Long enough to read from arm's length,
/// short enough that it is gone before it is in the way of the picture.
const NOTICE_SECONDS: f32 = 5.0;

/// One kept scene capture.
#[derive(Clone, Debug)]
pub struct SnapshotRow {
    pub entry: SnapshotEntry,
}

impl SnapshotRow {
    pub fn status(&self) -> String {
        self.entry.summary()
    }
}

enum Finished {
    Captured(Result<SnapshotEntry, String>),
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
            .map(|entry| SnapshotRow { entry })
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
            }
        }
    }

    /// Re-read the retained captures after the store applies its retention rule.
    fn reload(&mut self) {
        self.rows = self
            .store
            .list()
            .into_iter()
            .map(|entry| SnapshotRow { entry })
            .collect();
    }

    /// Whether a capture is still being written.
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
        }
    }

    #[test]
    fn a_saved_capture_lists_its_scene_without_an_export_action() {
        let row = SnapshotRow {
            entry: entry("14:22:08"),
        };
        assert_eq!(row.status(), "12 fixtures, 4 live");
    }

    #[test]
    fn a_capture_failure_stays_visible() {
        let mut snapshots = Snapshots::new(SnapshotStore::new(
            std::env::temp_dir().join("viz-snapshot-notice"),
            4,
        ));
        snapshots.announce("Snapshot could not be written", true);
        assert_eq!(
            snapshots.notice(),
            Some(("Snapshot could not be written", true))
        );
    }
}
