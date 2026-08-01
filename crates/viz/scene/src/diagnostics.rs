//! Visible connection, input, and fallback state. Every failure names its boundary so the
//! operator never sees a silent indefinite spinner.

use std::fmt;

/// Where the current connection stands. Every variant is presentable text.
#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Idle,
    Resolving {
        endpoint: String,
    },
    Authenticating {
        endpoint: String,
    },
    LoadingScene {
        endpoint: String,
    },
    Connected {
        endpoint: String,
        revision: u64,
    },
    /// The source answered and has no show in it: an editor whose operator has not opened a
    /// document yet, or a desk with nothing loaded. Nothing is wrong, and there is nothing to
    /// draw, so this is deliberately not a failure.
    WaitingForShow {
        endpoint: String,
    },
    /// A scene is still displayed but its configuration source is gone.
    Stale {
        endpoint: String,
        reason: String,
    },
    Failed {
        boundary: String,
        detail: String,
    },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// Whether the source is answering but has nothing loaded to draw.
    pub fn is_waiting_for_show(&self) -> bool {
        matches!(self, Self::WaitingForShow { .. })
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Idle => "Not connected".into(),
            Self::Resolving { endpoint } => format!("Resolving {endpoint}"),
            Self::Authenticating { endpoint } => format!("Authenticating with {endpoint}"),
            Self::LoadingScene { endpoint } => format!("Loading scene from {endpoint}"),
            Self::Connected { endpoint, revision } => {
                format!("Connected to {endpoint} (scene revision {revision})")
            }
            Self::WaitingForShow { endpoint } => format!("{endpoint} has no show loaded"),
            Self::Stale { endpoint, reason } => format!("{endpoint} stale: {reason}"),
            Self::Failed { boundary, detail } => format!("{boundary} failed: {detail}"),
        }
    }
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.summary())
    }
}

/// Why a fixture, model, or optical attribute fell back to generic behaviour.
#[derive(Clone, Debug, PartialEq)]
pub struct FallbackReason {
    pub boundary: &'static str,
    pub detail: String,
}

impl FallbackReason {
    pub fn new(boundary: &'static str, detail: impl Into<String>) -> Self {
        Self {
            boundary,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceProtocol {
    ArtNet,
    Sacn,
}

impl SourceProtocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net",
            Self::Sacn => "sACN",
        }
    }
}

/// Health of one configured input mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputHealth {
    /// Bound, but no valid frame has arrived yet.
    Waiting,
    Healthy,
    /// Protocol timeout or explicit stream termination.
    Stale,
    /// A higher-priority mapping owns this logical universe.
    Superseded,
    Failed,
}

impl InputHealth {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting for DMX",
            Self::Healthy => "Healthy",
            Self::Stale => "Stale",
            Self::Superseded => "Superseded",
            Self::Failed => "Failed",
        }
    }
}

/// Observable per-mapping counters.
#[derive(Clone, Debug)]
pub struct InputMappingStatus {
    pub mapping_id: String,
    pub protocol: SourceProtocol,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery: String,
    pub bind: String,
    pub health: InputHealth,
    pub last_packet_micros: Option<u64>,
    pub accepted_packets: u64,
    /// Frames this mapping received that were a second copy of one the universe had already
    /// taken. A show that routes one universe twice is working, and this says so plainly.
    pub duplicate_packets: u64,
    pub malformed_packets: u64,
    pub out_of_order_packets: u64,
    pub source_name: String,
    pub source_address: Option<String>,
    pub detail: String,
}

/// Operator-facing grade for one logical universe.
///
/// This lives on the provider boundary rather than in the DMX crate because the render core shows
/// it and a future source that is not Art-Net or sACN still has to answer the same question.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum UniverseGrade {
    /// Bound, but nothing has arrived yet.
    #[default]
    Waiting,
    /// Arriving at rate with no errors.
    Healthy,
    /// A rate drop, or at least one broken frame inside the health window.
    Degraded,
    /// Below the usable rate, or a second with heavy loss inside the health window.
    Critical,
}

impl UniverseGrade {
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Healthy => "ok",
            Self::Degraded => "degraded",
            Self::Critical => "critical",
        }
    }
}

/// Health of one logical universe, as the operator's status bar shows it.
#[derive(Clone, Copy, Debug, Default)]
pub struct UniverseHealth {
    pub universe: u16,
    pub rate_hz: f32,
    pub grade: UniverseGrade,
    pub accepted: u64,
    pub broken: u64,
    pub stale: bool,
    pub protocol: Option<SourceProtocol>,
}

/// Everything the connection surface must be able to show.
#[derive(Clone, Debug, Default)]
pub struct ProviderDiagnostics {
    pub endpoint: String,
    pub resolved_address: String,
    pub authenticated: bool,
    pub show_identity: String,
    pub scene_revision: u64,
    pub interface: String,
    pub inputs: Vec<InputMappingStatus>,
    /// One row per logical universe the show reads, ordered by universe number.
    pub universes: Vec<UniverseHealth>,
    pub warnings: Vec<String>,
}
