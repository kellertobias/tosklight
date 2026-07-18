use crate::*;
use light_core::{AttributeKey, FixtureId, Xyz};
use rusqlite::{Connection, params};
use std::collections::BTreeMap;
use std::fs;
use uuid::Uuid;

fn definition(footprint: u16) -> FixtureDefinition {
    FixtureDefinition {
        schema_version: 1,
        id: FixtureId::new(),
        revision: 1,
        manufacturer: "Test".into(),
        device_type: "other".into(),
        name: "Lamp".into(),
        model: "Lamp".into(),
        mode: "Mode".into(),
        footprint,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters: vec![],
        }],
        color_calibration: None,
        physical: FixturePhysicalProperties::default(),
        model_asset: None,
        icon_asset: None,
        hazardous: false,
        direct_control_protocols: Vec::new(),
        signal_loss_policy: SignalLossPolicy::HoldLast,
        safe_values: BTreeMap::new(),
        profile_id: None,
        mode_id: None,
        profile_snapshot: None,
    }
}

fn schema_v2_two_split_fixture() -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Test".into();
    profile.name = "Two split".into();
    let mode_id = profile.modes[0].id;
    profile.modes[0].splits.push(FixtureSplit {
        number: 2,
        footprint: 1,
    });
    profile.modes[0].heads.push(FixtureHead {
        id: Uuid::new_v4(),
        name: "Second".into(),
        master_shared: false,
    });
    let definition = profile.resolved_definition(mode_id).unwrap();
    PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Two split".into(),
        definition,
        universe: None,
        address: None,
        split_patches: vec![
            SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(1),
            },
            SplitPatch {
                split: 2,
                universe: None,
                address: None,
            },
        ],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![MultiPatchInstance {
            id: Uuid::new_v4(),
            name: "Second body".into(),
            universe: None,
            address: None,
            split_patches: vec![
                SplitPatch {
                    split: 1,
                    universe: Some(1),
                    address: Some(10),
                },
                SplitPatch {
                    split: 2,
                    universe: None,
                    address: None,
                },
            ],
            location: Default::default(),
            rotation: Default::default(),
        }],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    }
}

mod library;
mod migration;
mod model_encoding;
mod patch_validation;
