//! Installation-owned native-extension discovery, health, and rescan contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionDiagnostic {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionPackageSnapshot {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub directory: String,
    pub package_digest: Option<String>,
    pub readiness: String,
    pub locally_approved_unsigned: bool,
    pub diagnostics: Vec<ExtensionDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionInstanceSnapshot {
    pub id: String,
    pub extension_id: String,
    pub package_digest: String,
    pub executable: String,
    pub state: String,
    pub last_error: Option<String>,
    pub launches: u64,
    pub crashes: u64,
    pub protocol_errors: u64,
    pub inbound_drops: u64,
    pub outbound_drops: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionInstanceDiagnosticSnapshot {
    pub instance_id: String,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionRuntimeSnapshot {
    pub extensions_directory: String,
    pub configuration_path: String,
    pub configuration_diagnostic: Option<String>,
    pub packages: Vec<ExtensionPackageSnapshot>,
    pub instances: Vec<ExtensionInstanceSnapshot>,
    pub instance_diagnostics: Vec<ExtensionInstanceDiagnosticSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExtensionRescanRequest {
    pub request_id: String,
}
