//! TL-594 displayed-source lease and hold reason conversions between wire and application.
use light_application as application;
use light_wire::v2::{output_readouts as wire, visualization::VisualizationLane};

pub(super) fn displayed_source(
    source: wire::DisplayedSourceRef,
) -> application::ProgrammingDisplayedSource {
    application::ProgrammingDisplayedSource {
        lane: match source.lane {
            VisualizationLane::Normal => application::ProgrammingDisplayedLane::Normal,
            VisualizationLane::Preload => application::ProgrammingDisplayedLane::Preload,
        },
        lease: source.lease,
    }
}

pub(super) fn hold_reason(
    hold: application::ProgrammingValuesHold,
) -> wire::ProgrammingValuesHoldReason {
    match hold {
        application::ProgrammingValuesHold::DisplayedSourceUnavailable => {
            wire::ProgrammingValuesHoldReason::DisplayedSourceUnavailable
        }
        application::ProgrammingValuesHold::NativeColorUnavailable => {
            wire::ProgrammingValuesHoldReason::NativeColorUnavailable
        }
        application::ProgrammingValuesHold::ExplicitColorStartRequired => {
            wire::ProgrammingValuesHoldReason::ExplicitColorStartRequired
        }
        application::ProgrammingValuesHold::ZoomUnavailable => {
            wire::ProgrammingValuesHoldReason::ZoomUnavailable
        }
    }
}
