//! Capture-job observations shared by native hosts and renderer processes. No live-frame credit.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StillJobId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Ultra,
}
impl Engine {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Ultra => {
                "Ultra quality — regular ToskLight renderer; separate full-quality 4K image, with live quality unchanged"
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Phase {
    Queued,
    Rendering { detail: String },
    Writing,
    Cancelling { detail: String },
    Saved { path: PathBuf },
    Failed { detail: String },
    Cancelled,
}
impl Phase {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Saved { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub job_id: StillJobId,
    /// The host's invocation ID, echoed unchanged so delayed observations cannot answer a newer
    /// request. A host must still scope this to its helper-process attachment generation.
    pub request_id: Option<String>,
    pub pane_id: String,
    pub follow_preload: bool,
    pub scene_revision: u64,
    pub values_frame: u64,
    pub time_seconds: f32,
    pub width: u32,
    pub height: u32,
    pub framing: String,
    pub engine: Option<Engine>,
    /// Frozen-media availability or another actionable capture notice.
    pub notice: Option<String>,
    pub phase: Phase,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_capture_contract_preserves_typed_identity_and_phase() {
        assert!(Engine::Ultra.explanation().contains("ToskLight"));
        assert_eq!(serde_json::to_string(&Engine::Ultra).unwrap(), "\"ultra\"");
        let phase = Phase::Rendering {
            detail: "Native Ultra 3840×2160".into(),
        };
        let wire = postcard::to_allocvec(&phase).unwrap();
        assert_eq!(postcard::from_bytes::<Phase>(&wire).unwrap(), phase);
        assert_eq!(
            serde_json::to_string(&StillJobId("job-1".into())).unwrap(),
            "\"job-1\""
        );
        assert!(serde_json::from_str::<Engine>("\"ultimate\"").is_err());
    }
}
