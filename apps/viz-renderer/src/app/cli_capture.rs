//! One terminal CLI capture, claimed before readback or file I/O can block the event loop.
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct CliCapture {
    started: bool,
}

impl CliCapture {
    pub(super) fn started(&self) -> bool {
        self.started
    }

    pub(super) fn take_due(
        &mut self,
        requested: Option<&Path>,
        presented: u64,
        settle_frames: u32,
    ) -> Option<PathBuf> {
        if self.started || presented.saturating_add(1) < u64::from(settle_frames) {
            return None;
        }
        let path = requested?;
        self.started = true;
        // Keep Options::capture intact: it also suppresses preference writes for scripted runs.
        Some(path.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_redraws_cannot_overwrite_a_successful_or_failed_capture() {
        for capture_succeeds in [true, false] {
            let requested = Some(PathBuf::from("operator-requested.png"));
            let mut capture = CliCapture::default();
            assert!(capture.take_due(requested.as_deref(), 78, 80).is_none());
            assert!(!capture.started());
            let mut writes = Vec::new();
            for presented in [79, 79, 80, 100] {
                if let Some(path) = capture.take_due(requested.as_deref(), presented, 80) {
                    assert!(
                        capture.started(),
                        "ownership must precede blocking readback"
                    );
                    writes.push((path, capture_succeeds));
                }
            }
            assert_eq!(writes, vec![(requested.clone().unwrap(), capture_succeeds)]);
            assert_eq!(
                requested.as_deref(),
                Some(Path::new("operator-requested.png"))
            );
        }
    }

    #[test]
    fn absent_request_does_not_stop_an_ordinary_live_window() {
        let mut capture = CliCapture::default();
        assert!(capture.take_due(None, u64::MAX, 0).is_none());
        assert!(!capture.started());
        assert_eq!(
            capture.take_due(Some(Path::new("first.png")), 0, 0),
            Some(PathBuf::from("first.png"))
        );
        assert!(capture.started());
    }
}
