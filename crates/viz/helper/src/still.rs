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
    Ultimate,
}
impl Engine {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Ultra => {
                "Ultra quality — regular ToskLight renderer; separate full-quality 4K image, with live quality unchanged"
            }
            Self::Ultimate => {
                concat!(
                    "Ultimate quality — Blender/Cycles offline renderer; may take substantially longer. ",
                    "Known differences: aluminium, multiplex and fabric procedural finishes use plain PBR materials; ",
                    "floor grid and selection guides are omitted; native bloom is not reproduced. ",
                    "Cycles physical beam lighting, source softness, fog transport, global illumination and AgX tone mapping can differ. ",
                    "Choose Ultra for the native ToskLight appearance."
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Phase {
    Queued,
    CheckingBlender,
    NeedsChoice {
        blender_version: String,
        ultimate_reasons: Vec<String>,
        ultra_explanation: String,
        ultimate_explanation: String,
    },
    Rendering {
        detail: String,
    },
    Writing,
    Cancelling {
        detail: String,
    },
    Saved {
        path: PathBuf,
    },
    Failed {
        detail: String,
    },
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
    /// E.g. unusable Blender and automatic Ultra fallback. Never silently discard this notice.
    pub notice: Option<String>,
    pub phase: Phase,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ultimate_chooser_discloses_known_native_appearance_differences() {
        let explanation = Engine::Ultimate.explanation();
        for detail in [
            "aluminium, multiplex and fabric",
            "plain PBR",
            "floor grid and selection guides",
            "native bloom is not reproduced",
            "physical beam lighting",
            "fog transport",
            "AgX",
            "Choose Ultra",
        ] {
            assert!(
                explanation.contains(detail),
                "missing chooser disclosure: {detail}"
            );
        }
    }

    #[test]
    fn chooser_contract_names_engines_and_preserves_typed_job_identity() {
        let phase = Phase::NeedsChoice {
            blender_version: "test".into(),
            ultimate_reasons: vec!["gobo".into()],
            ultra_explanation: Engine::Ultra.explanation().into(),
            ultimate_explanation: Engine::Ultimate.explanation().into(),
        };
        let value = serde_json::to_value(&phase).unwrap();
        assert_eq!(value["needsChoice"]["ultimateReasons"][0], "gobo");
        assert!(
            value["needsChoice"]["ultraExplanation"]
                .as_str()
                .unwrap()
                .contains("ToskLight")
        );
        assert!(
            value["needsChoice"]["ultimateExplanation"]
                .as_str()
                .unwrap()
                .contains("Blender")
        );
        let wire = postcard::to_allocvec(&phase).unwrap();
        assert_eq!(postcard::from_bytes::<Phase>(&wire).unwrap(), phase);
        assert_eq!(
            serde_json::to_string(&StillJobId("job-1".into())).unwrap(),
            "\"job-1\""
        );
    }
}
