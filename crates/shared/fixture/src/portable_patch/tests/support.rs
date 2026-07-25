use super::super::{ResolvedFixtureProfileRevision, fixture_profile_content_digest};
use crate::{
    ChannelBehavior, ChannelResolution, DirectControlEndpoint, DirectControlProtocol,
    FixtureChannel, FixtureHead, FixtureLocation, FixtureProfile, FixtureVector,
    MultiPatchInstance, PatchedFixture, PatchedHead, SplitPatch,
};
use light_core::{AttributeKey, FixtureId};
use serde_json::Value;
use uuid::Uuid;

pub(super) fn profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.revision = 7;
    profile.manufacturer = "Acme".into();
    profile.name = "Portable wash".into();
    profile.short_name = "Wash".into();
    profile.direct_control_protocols = vec![DirectControlProtocol::Citp];
    let mode = &mut profile.modes[0];
    mode.name = "Touring".into();
    let primary_head = mode.heads[0].id;
    mode.heads.push(FixtureHead {
        id: Uuid::new_v4(),
        name: "Cell".into(),
        master_shared: false,
    });
    mode.channels.push(FixtureChannel {
        id: Uuid::new_v4(),
        head_id: primary_head,
        split: 1,
        attribute: AttributeKey("intensity".into()),
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: Some(0.0),
        physical_max: Some(100.0),
        unit: Some("percent".into()),
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: vec![],
    });
    profile
}

pub(super) fn fixture(profile: &FixtureProfile) -> PatchedFixture {
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let child_id = FixtureId::new();
    let multipatch_id = Uuid::new_v4();
    let highlight_overrides = profile.modes[0]
        .channels
        .first()
        .map(|channel| (channel.id, 187))
        .into_iter()
        .collect();
    PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(42),
        virtual_fixture_number: None,
        name: "Stage left".into(),
        definition,
        universe: Some(2),
        address: Some(101),
        split_patches: vec![SplitPatch {
            split: 1,
            universe: Some(2),
            address: Some(101),
        }],
        layer_id: "overhead".into(),
        direct_control: Some(DirectControlEndpoint {
            protocol: DirectControlProtocol::Citp,
            ip_address: "192.0.2.25".parse().unwrap(),
            port: 4812,
        }),
        location: FixtureLocation {
            x: 1_200,
            y: -350,
            z: 4_800,
        },
        rotation: FixtureVector {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        },
        logical_heads: vec![PatchedHead {
            profile_head_id: None,
            head_index: 1,
            fixture_id: child_id,
        }],
        multipatch: vec![MultiPatchInstance {
            id: multipatch_id,
            name: "Balcony".into(),
            universe: Some(3),
            address: Some(201),
            split_patches: vec![SplitPatch {
                split: 1,
                universe: Some(3),
                address: Some(201),
            }],
            location: FixtureLocation {
                x: -2_000,
                y: 0,
                z: 3_500,
            },
            rotation: FixtureVector {
                x: 0.0,
                y: 90.0,
                z: 0.0,
            },
        }],
        move_in_black_enabled: false,
        move_in_black_delay_millis: 275,
        highlight_overrides,
    }
}

pub(super) fn source(profile: &FixtureProfile) -> ResolvedFixtureProfileRevision {
    source_from_value(serde_json::to_value(profile).unwrap())
}

pub(super) fn source_from_value(profile: Value) -> ResolvedFixtureProfileRevision {
    let profile_id = FixtureId(Uuid::parse_str(profile["id"].as_str().unwrap()).unwrap());
    let profile_revision = profile["revision"].as_u64().unwrap();
    let digest = fixture_profile_content_digest(&profile).unwrap();
    ResolvedFixtureProfileRevision::new(profile_id, profile_revision, digest, profile)
}
