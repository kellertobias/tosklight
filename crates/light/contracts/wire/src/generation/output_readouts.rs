//! TL-594 readout and displayed-source declarations. Kept beside `declarations.rs`, which is
//! already at the file-size limit.
use ts_rs::{Config, TS};

use crate::v2::output_readouts::*;

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        DisplayedSourceRef::decl(config),
        OutputReadoutUnavailable::decl(config),
        ProgrammingValuesHoldReason::decl(config),
        OutputPositionCommandReadout::decl(config),
        OutputCommonAngles::decl(config),
        OutputPositionReadout::decl(config),
        OutputOwnerReadout::decl(config),
        OutputReadoutSnapshot::decl(config),
        VisualizationReadoutClaim::decl(config),
        VisualizationPendingStamp::decl(config),
    ]
}
