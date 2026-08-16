//! Importing media into the library.
//!
//! Import is long-running, so it is a *job*: the request returns identities and the work reports
//! itself afterwards. A connection is never held open for the length of a transcode, and nothing
//! invents a percentage it cannot compute — a source whose frame count FFmpeg did not report has
//! no fraction, and says so by leaving it absent.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::FolderPresentation;
use crate::diagnostics::{ImportJob, ImportOutcome, PendingImport};
use crate::wire::AddressView;

/// A file sitting in the library that could be played once it has been imported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PendingImportView {
    pub address: AddressView,
    /// The name the imported clip will keep.
    pub name: String,
    /// The filename as it sits on disk, so an operator recognises what they are about to convert.
    pub filename: String,
}

impl PendingImportView {
    pub fn of(pending: &PendingImport) -> Self {
        Self {
            address: AddressView::of(pending.destination),
            name: pending.name.clone(),
            filename: pending.filename.clone(),
        }
    }
}

/// One import, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobView {
    pub id: String,
    pub address: AddressView,
    pub filename: String,
    /// `queued`, `running`, `succeeded`, `failed`, or `cancelled`.
    pub state: String,
    /// Absent while the total is unknown, rather than a made-up number.
    pub fraction: Option<f32>,
    pub frames_done: Option<u32>,
    pub frames_total: Option<u32>,
    /// Why it failed, when it did. Operator-facing text.
    pub reason: Option<String>,
}

impl ImportJobView {
    pub fn of(job: &ImportJob) -> Self {
        let (state, reason) = match &job.outcome {
            ImportOutcome::Queued => ("queued", None),
            ImportOutcome::Running => ("running", None),
            ImportOutcome::Succeeded => ("succeeded", None),
            ImportOutcome::Failed { reason } => ("failed", Some(reason.clone())),
            ImportOutcome::Cancelled => ("cancelled", None),
        };
        Self {
            id: job.id.clone(),
            address: AddressView::of(job.destination),
            filename: job.filename.clone(),
            state: state.to_owned(),
            fraction: job.fraction,
            frames_done: job.frames_done,
            frames_total: job.frames_total,
            reason,
        }
    }
}

/// What the library import panel reads before its telemetry socket is up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportsView {
    pub pending: Vec<PendingImportView>,
    pub jobs: Vec<ImportJobView>,
    /// Whether this machine can transcode at all. Import shells out to FFmpeg, and a machine
    /// without it should say so before an operator queues forty clips that will all fail.
    pub can_import: bool,
}

/// Starting an import.
///
/// Absent `folder` and `file` mean everything waiting, which is the whole point on a library
/// carried over from the previous Media Server. Naming one imports just that.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartImport {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<u8>,
}

/// Renaming or moving one stable catalog item.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryItem {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<u8>,
    /// Correct or clear the authored tempo. An absent field leaves it unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_bpm: Option<Option<f64>>,
    /// Exchange addresses when the destination is occupied. False refuses the edit.
    #[serde(default)]
    pub swap: bool,
}

/// Setting the visible label for one numbered folder. An empty name removes the label.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryFolder {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_with: Option<u16>,
}

/// Shared presentation for a media, text, or generated-visualizer folder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct FolderPresentationView {
    pub folder: u16,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub picture_url: Option<String>,
}

impl FolderPresentationView {
    pub fn of(presentation: &FolderPresentation) -> Self {
        Self {
            folder: presentation.folder,
            name: presentation.name.clone(),
            icon: presentation.icon.clone(),
            picture_url: presentation.picture_content_type.as_ref().map(|_| {
                format!(
                    "/api/v2/folder-presentations/{}/picture",
                    presentation.folder
                )
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct FolderPresentationsView {
    pub folders: Vec<FolderPresentationView>,
}

/// Intent update. Empty strings clear name/icon; absent fields leave them unchanged.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFolderPresentation {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoveFolderPicture {
    pub request_id: String,
}

/// The immediate answer after a browser upload has been accepted and queued.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UploadAcceptedView {
    pub job_id: String,
    pub address: AddressView,
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::MediaAddress;

    #[test]
    fn a_running_job_reports_how_far_along_it_is() {
        let view = ImportJobView::of(&ImportJob {
            id: "job-1".to_owned(),
            destination: MediaAddress::new(1, 4),
            filename: "004-LoopTest.mp4".to_owned(),
            outcome: ImportOutcome::Running,
            fraction: Some(0.25),
            frames_done: Some(25),
            frames_total: Some(100),
        });

        assert_eq!(view.state, "running");
        assert_eq!(view.fraction, Some(0.25));
        assert_eq!(view.address.folder, 1);
        assert_eq!(view.reason, None);
    }

    #[test]
    fn a_source_with_no_frame_count_gets_no_invented_percentage() {
        let view = ImportJobView::of(&ImportJob {
            id: "job-2".to_owned(),
            destination: MediaAddress::new(1, 1),
            filename: "001.mov".to_owned(),
            outcome: ImportOutcome::Running,
            fraction: None,
            frames_done: Some(40),
            frames_total: None,
        });

        assert_eq!(view.fraction, None);
        assert_eq!(view.frames_done, Some(40));
    }

    #[test]
    fn a_failed_job_keeps_its_reason_for_somebody_to_read() {
        let view = ImportJobView::of(&ImportJob {
            id: "job-3".to_owned(),
            destination: MediaAddress::new(2, 1),
            filename: "001-Unknown.png".to_owned(),
            outcome: ImportOutcome::Failed {
                reason: "FFmpeg is not installed or not on PATH".to_owned(),
            },
            fraction: None,
            frames_done: None,
            frames_total: None,
        });

        assert_eq!(view.state, "failed");
        assert!(view.reason.is_some_and(|reason| reason.contains("FFmpeg")));
    }

    #[test]
    fn importing_everything_waiting_names_no_address() {
        let body: StartImport = serde_json::from_str(r#"{"requestId":"a"}"#).unwrap();
        assert_eq!(body.folder, None);
        assert_eq!(body.file, None);

        let one: StartImport =
            serde_json::from_str(r#"{"requestId":"a","folder":1,"file":4}"#).unwrap();
        assert_eq!(one.folder, Some(1));
        assert_eq!(one.file, Some(4));
    }
}
