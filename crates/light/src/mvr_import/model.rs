use crate::{ActionContext, ApplicationCommand, CommandFamily, PatchChange, PreparedShowCandidate};
use light_core::ShowId;
use light_fixture::FixtureDefinition;
use light_show::{PortablePatchRevision, PortableShowRevision};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MvrImportResolution {
    Import,
    Skip,
    ImportUnpatched,
    Replace,
    Address { universe: u16, address: u16 },
}

/// Trusted import request assembled after the transport has decoded and bounded the MVR archive.
#[derive(Clone, Debug)]
pub struct ApplyActiveMvrImportCommand {
    pub show_id: ShowId,
    pub document: light_mvr::MvrDocument,
    pub definitions: Vec<FixtureDefinition>,
    pub resolutions: HashMap<Uuid, MvrImportResolution>,
}

impl ApplicationCommand for ApplyActiveMvrImportCommand {
    type Value = ActiveMvrImportResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveMvrImportResult {
    pub context: ActionContext,
    pub changed: bool,
    pub show_revision: PortableShowRevision,
    pub patch_revision: PortablePatchRevision,
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
    pub change: PatchChange,
    pub event_sequence: Option<u64>,
}

/// Opaque, compiled candidate tied to the exact active-show revision from which it was planned.
pub struct PreparedActiveMvrImport {
    pub(super) show_id: ShowId,
    pub(super) source_show_revision: PortableShowRevision,
    pub(super) source_patch_revision: PortablePatchRevision,
    pub(super) candidate: Option<Box<PreparedShowCandidate>>,
    pub(super) state: PreparedMvrImportState,
}

pub(super) struct PreparedMvrImportState {
    pub context: ActionContext,
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
    pub patch: PlannedPatchChange,
}

#[derive(Default)]
pub(super) struct PlannedPatchChange {
    pub fixtures: Vec<PlannedFixture>,
    pub removed_fixture_ids: Vec<light_core::FixtureId>,
    pub profiles: Vec<crate::PatchProfileRevisionProjection>,
}

pub(super) struct PlannedFixture {
    pub profile: light_fixture::PatchedFixtureProfileReference,
    pub patch: light_fixture::PatchedFixturePatch,
    pub profile_projection: crate::PatchProfileRevisionProjection,
}
