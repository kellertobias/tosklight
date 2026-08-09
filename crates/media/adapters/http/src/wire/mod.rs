//! The typed wire contract.
//!
//! Request types are tolerant of unknown fields; response types are explicit. Nothing here is a
//! hand-built JSON string, and nothing re-derives a domain rule — the API projects state, it does
//! not decide it.
//!
//! Every type here is a *projection*, owned by this adapter. Domain types are deliberately not
//! serialized straight onto the wire: a domain refactor must not silently become a breaking API
//! change, and the TypeScript the frontend consumes is generated from exactly these declarations
//! by `cargo run -p media-http --example generate-contracts`.
//!
//! One module per subject, because the contract is read far more often than it is written and an
//! operator-facing panel is easier to follow when its types sit together.

mod audio;
mod catalog;
mod dmx;
mod health;
mod library;
mod logs;
mod network;
mod output;
mod text;
mod visualizer;

pub use audio::{
    AudioBandsView, AudioEditError, AudioPanelView, AudioSettingsView, AudioView, TelemetryFrame,
    UpdateAudio, WaveformView,
};
pub use catalog::{CatalogFolderView, CatalogItemView, CatalogView};
pub use dmx::{
    DmxChannelGroupView, DmxChannelView, DmxIngressView, DmxMapView, DmxPersonalityView,
    DmxResolutionView, DmxValueSetView,
};
pub use health::{AddressView, Health, SourceStatusView};
pub use library::{
    ImportJobView, ImportsView, PendingImportView, StartImport, UpdateLibraryFolder,
    UpdateLibraryItem, UploadAcceptedView,
};
pub use logs::{LogRecordView, LogsView};
pub use network::{NetworkAddressesView, NetworkEditError, NetworkView, UpdateNetwork};
pub use output::{
    LayerView, MaskView, MasterView, OutputConfigurationEditError, OutputConfigurationView,
    OutputView, UpdateLayer, UpdateOutputConfiguration,
};
pub use text::{CreateText, DeleteText, TextEditError, TextSlotView, TextStyleView, UpdateText};
pub use visualizer::{UpdateVisualizer, VisualizerParametersView, VisualizerView};
