//! TL-554 Direct (native) Color page, edit-option and status declarations and schema. Kept beside
//! `declarations.rs`, which is already at the file-size limit.
use schemars::generate::SchemaSettings;
use ts_rs::{Config, TS};

use crate::v2::native_color::*;

use super::{GeneratedArtifact, namespaced_schema};

const SCHEMA_DIRECTORY: &str = "crates/light/contracts/wire/schemas/v2-programming";

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        NativeColorReferenceRef::decl(config),
        ExplicitColorStart::decl(config),
        ColorAdoptionStart::decl(config),
        ColorAdoptionFixture::decl(config),
        ColorAdoptionReport::decl(config),
        NativeColorResolution::decl(config),
        NativeColorFunctionDescriptor::decl(config),
        NativeColorControlDescriptor::decl(config),
        NativeColorPage::decl(config),
        NativeColorReference::decl(config),
        NativeColorReferenceCandidate::decl(config),
        NativeColorReplayPreview::decl(config),
        NativeColorFixturePreview::decl(config),
        NativeColorPagesUnavailable::decl(config),
        NativeColorValueReadout::decl(config),
        NativeColorValues::decl(config),
        NativeColorPagesSnapshot::decl(config),
        ColorIntentDirectReplay::decl(config),
        ColorIntentDirectCompatibility::decl(config),
        ColorIntentDirectUv::decl(config),
        ColorIntentDirectOrigin::decl(config),
        ColorIntentDriveLimit::decl(config),
        ColorIntentDirectReport::decl(config),
    ]
}

pub(super) fn artifacts() -> Vec<GeneratedArtifact> {
    vec![namespaced_schema::<NativeColorPagesSnapshot>(
        SCHEMA_DIRECTORY,
        "native-color-pages-snapshot",
        SchemaSettings::draft2020_12().for_serialize(),
    )]
}
