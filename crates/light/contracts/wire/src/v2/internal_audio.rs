//! Internal Audio Player availability and library diagnostics.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct InternalAudioStatus {
    pub players: Vec<InternalAudioPlayerStatus>,
    pub libraries: Vec<InternalAudioLibraryStatus>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct InternalAudioPlayerStatus {
    pub fixture_id: Uuid,
    pub available: bool,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct InternalAudioLibraryStatus {
    pub binding: String,
    pub entries: usize,
    pub diagnostics: Vec<String>,
}
