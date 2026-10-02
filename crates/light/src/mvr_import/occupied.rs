//! Resolve portable profile references once for address occupancy; invalid records cannot vanish.
use crate::{ActionError, ActionErrorKind};
use light_fixture::{
    PatchedFixtureCompiler, PortablePatchedFixtureRecord, ResolvedFixtureProfileRevision,
};
use light_show::PortableShowDocument;

pub type OccupiedPatch = (u16, u16, u16, String);

pub fn occupied_patches(
    document: &PortableShowDocument,
) -> Result<Vec<OccupiedPatch>, ActionError> {
    let legacy = document
        .canonical_legacy_fixture_profile_revisions()
        .map_err(invalid)?;
    for profile in &legacy {
        if let Some(current) =
            document.fixture_profile_revision(profile.id().profile_id(), profile.id().revision())
        {
            if current.digest() != profile.digest() {
                return Err(invalid("inline and retained profile revisions conflict"));
            }
        }
    }
    let mut compiler =
        PatchedFixtureCompiler::new(|reference: light_fixture::PatchedFixtureProfileReference| {
            document
                .fixture_profile_revision(reference.profile_id, reference.profile_revision)
                .or_else(|| {
                    legacy.iter().find(|profile| {
                        profile.id().profile_id() == reference.profile_id
                            && profile.id().revision() == reference.profile_revision
                    })
                })
                .map(|profile| {
                    ResolvedFixtureProfileRevision::new(
                        profile.id().profile_id(),
                        profile.id().revision(),
                        profile.digest().as_str(),
                        profile.profile().clone(),
                    )
                })
        });
    let mut occupied = Vec::new();
    for object in document.objects_of_kind("patched_fixture") {
        let record =
            PortablePatchedFixtureRecord::decode(object.body().clone()).map_err(invalid)?;
        let fixture = compiler.compile(&record).map_err(invalid)?;
        occupied.extend(fixture_occupied_patches(&fixture, object.key().id()));
    }
    Ok(occupied)
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(
        ActionErrorKind::Invalid,
        format!("Cannot determine existing MVR address occupancy: {error}"),
    )
}

/// All physical copies and DMX splits belong to the parent show fixture for conflict handling.
pub fn fixture_occupied_patches(
    fixture: &light_fixture::PatchedFixture,
    id: &str,
) -> Vec<OccupiedPatch> {
    let footprints = fixture.definition.split_footprints();
    let primary = footprints.keys().next().copied().unwrap_or(1);
    let patches = |explicit: &[light_fixture::SplitPatch], universe, address| {
        if explicit.is_empty() {
            vec![light_fixture::SplitPatch {
                split: primary,
                universe,
                address,
            }]
        } else {
            explicit.to_vec()
        }
    };
    let mut rows = Vec::new();
    for patch in patches(&fixture.split_patches, fixture.universe, fixture.address)
        .into_iter()
        .chain(
            fixture
                .multipatch
                .iter()
                .flat_map(|copy| patches(&copy.split_patches, copy.universe, copy.address)),
        )
    {
        if let (Some((universe, address)), Some(footprint)) = (
            patch.universe.zip(patch.address),
            footprints.get(&patch.split),
        ) {
            rows.push((universe, address, *footprint, id.to_owned()));
        }
    }
    rows
}

/// MVR carries a primary address. Native secondary splits survive; new ones stay unpatched.
/// An explicit/unresolved unpatched choice must suppress every physical output of the fixture.
pub fn apply_mvr_primary_address(
    fixture: &mut light_fixture::PatchedFixture,
    address: (Option<u16>, Option<u16>),
    suppress_all_outputs: bool,
) {
    let footprints = fixture.definition.split_footprints();
    let old = std::mem::take(&mut fixture.split_patches);
    fixture.universe = address.0;
    fixture.address = address.1;
    fixture.split_patches = footprints
        .keys()
        .enumerate()
        .map(|(index, split)| {
            let (universe, slot) = if index == 0 {
                address
            } else if !suppress_all_outputs {
                old.iter()
                    .find(|patch| patch.split == *split)
                    .map_or((None, None), |patch| (patch.universe, patch.address))
            } else {
                (None, None)
            };
            light_fixture::SplitPatch {
                split: *split,
                universe,
                address: slot,
            }
        })
        .collect();
    if suppress_all_outputs {
        for copy in &mut fixture.multipatch {
            copy.universe = None;
            copy.address = None;
            copy.split_patches = footprints
                .keys()
                .map(|split| light_fixture::SplitPatch {
                    split: *split,
                    universe: None,
                    address: None,
                })
                .collect();
        }
    }
}

pub fn primary_footprint(definition: &light_fixture::FixtureDefinition) -> u16 {
    definition
        .split_footprints()
        .values()
        .next()
        .copied()
        .unwrap_or(definition.footprint)
}
