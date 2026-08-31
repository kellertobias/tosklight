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
mod effect;
mod health;
mod library;
mod logs;
mod network;
mod output;
mod output_edit;
mod pixel_map;
mod text;
mod time;
mod visualizer;

pub use audio::{
    AudioBandsView, AudioEditError, AudioPanelView, AudioSettingsView, AudioView, DeskIdentityView,
    TelemetryFrame, UpdateAudio, WaveformView,
};
pub use catalog::{CatalogFolderView, CatalogItemView, CatalogView};
pub use dmx::{
    DmxChannelGroupView, DmxChannelView, DmxIngressView, DmxMapView, DmxPersonalityView,
    DmxResolutionView, DmxValueSetView,
};
pub use effect::{EffectParameterView, EffectSlotView};
pub use health::{AddressView, Health, RunningOutputView, RunningServerView, SourceStatusView};
pub use library::{
    FolderPresentationView, FolderPresentationsView, ImportJobView, ImportsView, PendingImportView,
    RemoveFolderPicture, StartImport, UpdateFolderPresentation, UpdateLibraryFolder,
    UpdateLibraryItem, UploadAcceptedView,
};
pub use logs::{LogRecordView, LogsView, ServerLogLevelView, UpdateServerLogLevel};
pub use network::{NetworkAddressesView, NetworkEditError, NetworkView, UpdateNetwork};
pub use output::{
    AvailableMonitorView, LayerView, MaskView, MasterView, OutputConfigurationValuesView,
    OutputConfigurationView, OutputView, UpdateLayer, UpdateMaster,
};
pub use output_edit::{OutputConfigurationEditError, UpdateOutputConfiguration};
pub use pixel_map::{
    CanvasPointView, DisplayRegionView, PixelLayoutView, PixelMapEditError, PixelMapView,
    PixelRouteView, PixelZoneHandoffView, PixelZoneView,
};
pub use text::{
    CreateText, DeleteText, TextEditError, TextFormatView, TextSlotView, TextStyleView, UpdateText,
};
pub use time::{TimeEditError, TimeView, UpdateTime};
pub use visualizer::{
    CreateVisualizer, UpdateVisualizer, VisualizerParametersView, VisualizerView,
};
