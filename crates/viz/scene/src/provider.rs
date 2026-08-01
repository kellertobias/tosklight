//! The source-provider boundary.
//!
//! A provider owns its transport, authentication, packet decoding, and reconnect behaviour, and
//! publishes only the semantic events below. The render core never learns which provider is
//! active, so a lighting desk and a future local planning application project into the same
//! renderer scene without touching GPU code.

use crate::diagnostics::{ConnectionState, ProviderDiagnostics};
use crate::scene::Scene;
use crate::values::SceneValues;
use crate::view::ViewConfiguration;
use std::fmt;

/// Which source is selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    LightingDesk,
    PlanningSoftware,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::LightingDesk => "Lighting desk",
            Self::PlanningSoftware => "Planning software",
        }
    }

    /// The stable spelling, for anything written down and read back.
    pub fn wire(self) -> &'static str {
        match self {
            Self::LightingDesk => "lighting_desk",
            Self::PlanningSoftware => "planning_software",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "lighting_desk" => Some(Self::LightingDesk),
            "planning_software" => Some(Self::PlanningSoftware),
            _ => None,
        }
    }
}

/// What a built provider can actually do, so the operator surface never offers a source that
/// this build cannot connect.
#[derive(Clone, Debug)]
pub struct ProviderCapabilities {
    pub kind: ProviderKind,
    pub available: bool,
    /// Shown next to an unavailable source, for example "Not available in this build".
    pub unavailable_reason: Option<String>,
    pub default_host: String,
    pub default_port: u16,
    /// Whether this provider consumes network DMX for its live values.
    pub uses_network_input: bool,
}

/// One semantic message from the active provider.
#[derive(Debug)]
pub enum ProviderEvent {
    /// Connection lifecycle changed. Always presentable.
    Connection(ConnectionState),
    /// A complete, already validated scene replaces the displayed one atomically.
    Snapshot {
        scene: Box<Scene>,
        /// The source's authoritative view, when it has one. `None` means the source has no
        /// opinion and whatever the operator selected locally stays selected.
        view: Option<ViewConfiguration>,
    },
    /// Structural change to an existing scene. Applied without a full rebuild.
    SceneDelta(Box<Scene>),
    /// Newest live values. Never carries geometry.
    Values(Box<SceneValues>),
    /// Authoritative view configuration from the source.
    View(ViewConfiguration),
    /// Diagnostics refresh.
    Diagnostics(Box<ProviderDiagnostics>),
    /// The provider needs the host to discard deltas and wait for a fresh snapshot.
    ResyncRequired { reason: String },
}

#[derive(Debug)]
pub struct ProviderError {
    pub boundary: &'static str,
    pub detail: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(boundary: &'static str, detail: impl Into<String>, retryable: bool) -> Self {
        Self {
            boundary,
            detail: detail.into(),
            retryable,
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.boundary, self.detail)
    }
}

impl std::error::Error for ProviderError {}

/// A running scene source. Implementations push [`ProviderEvent`]s and are polled without
/// blocking the render loop.
pub trait SceneProvider: Send {
    fn capabilities(&self) -> ProviderCapabilities;

    /// Drain everything the provider has produced since the last call. Must never block.
    fn poll(&mut self) -> Vec<ProviderEvent>;

    /// Ask for a fresh full snapshot, for example after a revision gap.
    fn request_resync(&mut self);

    /// Stop the provider and release its sockets and tasks.
    fn shutdown(&mut self);
}
