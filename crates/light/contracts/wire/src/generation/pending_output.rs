//! TL-594 track B: declarations for the accepted Pending Preload status and the accepted-frame
//! colour report, and (TL-550) the per-head UV result of that report. Kept beside `declarations.rs`, which is already at the file-size limit.
use ts_rs::{Config, TS};

use crate::v2::attribute_configuration::{
    ColorIntentAcceptedFrame, ColorIntentFrameState, ColorIntentUvReport, ColorIntentUvStatus,
};
use crate::v2::output_control::{OutputPreloadState, OutputPreloadStatus};

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        OutputPreloadStatus::decl(config),
        OutputPreloadState::decl(config),
        ColorIntentAcceptedFrame::decl(config),
        ColorIntentFrameState::decl(config),
        ColorIntentUvReport::decl(config),
        ColorIntentUvStatus::decl(config),
    ]
}
