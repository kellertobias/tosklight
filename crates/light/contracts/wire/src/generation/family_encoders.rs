//! Semantic family encoder page declarations and schema (TL-549/550/551). Kept beside
//! `declarations.rs`, which is already at the file-size limit.
use schemars::generate::SchemaSettings;
use ts_rs::{Config, TS};

use crate::v2::family_encoders::*;

use super::{GeneratedArtifact, namespaced_schema};

const SCHEMA_DIRECTORY: &str = "crates/light/contracts/wire/schemas/v2-programming";

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        ColorEncoderPresentation::decl(config),
        FamilyEncoderFamily::decl(config),
        FamilyEncoderLimitsSource::decl(config),
        FamilyEncoderEditKind::decl(config),
        FamilyEncoderComponentSlot::decl(config),
        FamilyEncoderSlot::decl(config),
        FamilyEncoderPage::decl(config),
        FamilyEncoderReservation::decl(config),
        FamilyEncoderReservedPage::decl(config),
        FamilyEncoderGroup::decl(config),
        FamilyEncoderPagesSnapshot::decl(config),
    ]
}

pub(super) fn artifacts() -> Vec<GeneratedArtifact> {
    vec![namespaced_schema::<FamilyEncoderPagesSnapshot>(
        SCHEMA_DIRECTORY,
        "family-encoder-pages-snapshot",
        SchemaSettings::draft2020_12().for_serialize(),
    )]
}
