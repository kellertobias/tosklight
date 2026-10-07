//! Which shared lane projection a stream claim reads: lane, Preload session and detail.
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::runtime) enum VisualizationProjectionKey {
    Normal {
        include_dynamic_stack: bool,
        /// Every resolved attribute rather than only those the Stage draws.
        complete_values: bool,
    },
    Preload {
        session_id: Uuid,
        include_dynamic_stack: bool,
        complete_values: bool,
    },
}

impl VisualizationProjectionKey {
    pub(in crate::runtime) const fn includes_dynamic_stack(self) -> bool {
        match self {
            Self::Normal {
                include_dynamic_stack,
                ..
            }
            | Self::Preload {
                include_dynamic_stack,
                ..
            } => include_dynamic_stack,
        }
    }

    pub(in crate::runtime) const fn complete_values(self) -> bool {
        match self {
            Self::Normal {
                complete_values, ..
            }
            | Self::Preload {
                complete_values, ..
            } => complete_values,
        }
    }
}
