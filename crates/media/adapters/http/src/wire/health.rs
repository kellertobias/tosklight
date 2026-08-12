//! Process health, addresses, and source status.
//!
//! The small projections several other views embed: what a client needs to label a selection or
//! render a status badge without re-implementing a domain rule.

use media_domain::{MediaAddress, SourceFailure, SourceStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the process is up and what it is running.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub status: String,
    pub instance: String,
    pub outputs: usize,
    /// A revision counter, not an identifier. It stays inside the range a browser can hold in a
    /// number, so the client is not forced into `bigint` arithmetic to compare two snapshots.
    #[ts(type = "number")]
    pub catalog_revision: u64,
    pub catalog_items: usize,
}

/// A `(folder, file)` selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddressView {
    pub folder: u8,
    pub file: u8,
    /// Which address space the pair falls in, so the UI can label a selection without
    /// re-implementing the ranges.
    pub class: String,
}

impl AddressView {
    pub fn of(address: MediaAddress) -> Self {
        Self {
            folder: address.folder,
            file: address.file,
            class: match address.classify() {
                media_domain::AddressClass::Blank => "blank",
                media_domain::AddressClass::Library => "library",
                media_domain::AddressClass::TextBank => "text-bank",
                media_domain::AddressClass::GeneratedVisualizer => "generated-visualizer",
            }
            .to_owned(),
        }
    }
}

/// A layer's source lifecycle, flattened for a client that only wants to render a badge.
///
/// `failure` carries operator-safe text only. Absolute paths and decoder internals stay in the
/// log, which is where someone diagnosing a machine looks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatusView {
    pub state: String,
    pub failure: Option<String>,
}

impl SourceStatusView {
    pub fn of(status: SourceStatus) -> Self {
        let (state, failure) = match status {
            SourceStatus::Unselected => ("unselected", None),
            SourceStatus::Loading => ("loading", None),
            SourceStatus::Ready => ("ready", None),
            SourceStatus::Completed => ("completed", None),
            SourceStatus::Failed { failure } => ("failed", Some(describe(failure))),
        };
        Self {
            state: state.to_owned(),
            failure,
        }
    }
}

/// Operator-safe text for a source failure.
///
/// The domain names the failure; wording it for a person is this adapter's job, and the wording
/// says what to do about it rather than what the decoder saw.
fn describe(failure: SourceFailure) -> String {
    match failure {
        SourceFailure::MissingFile => "the file is not in the library folder",
        SourceFailure::UnsupportedCodec => "this file is not in a playable format; import it",
        SourceFailure::DecodeFailed => "the file could not be decoded; it may be damaged",
        SourceFailure::GpuUploadFailed => "the graphics device rejected this frame",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_carries_the_space_it_falls_in() {
        assert_eq!(AddressView::of(MediaAddress::BLANK).class, "blank");
        assert_eq!(AddressView::of(MediaAddress::new(1, 1)).class, "library");
        assert_eq!(
            AddressView::of(MediaAddress::new(200, 1)).class,
            "text-bank"
        );
        assert_eq!(
            AddressView::of(MediaAddress::new(250, 1)).class,
            "generated-visualizer"
        );
    }

    #[test]
    fn a_failed_source_reports_operator_safe_text_and_nothing_else() {
        let ready = SourceStatusView::of(SourceStatus::Ready);
        assert_eq!(ready.state, "ready");
        assert_eq!(ready.failure, None);

        let failed = SourceStatusView::of(SourceStatus::Failed {
            failure: SourceFailure::MissingFile,
        });
        assert_eq!(failed.state, "failed");
        assert!(
            failed.failure.is_some_and(|text| !text.is_empty()),
            "a failed layer must say something an operator can act on"
        );
    }
}
