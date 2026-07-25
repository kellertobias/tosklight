use super::{
    FIXTURE_PROFILE_SCHEMA_VERSION, FixtureHead, FixtureMode, FixtureProfile, FixtureSplit,
    GeometryGraph, GeometryTemplate, ModelUnits, PatchPolicy, ProfilePhysicalProperties,
};
use crate::SignalLossPolicy;
use light_core::FixtureId;
use uuid::Uuid;

impl FixtureProfile {
    pub fn blank() -> Self {
        let profile_id = FixtureId::new();
        let mode_id = Uuid::new_v4();
        let head_id = Uuid::new_v4();
        Self {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: profile_id,
            revision: 0,
            manufacturer: String::new(),
            name: String::new(),
            short_name: String::new(),
            fixture_type: "other".into(),
            patch_policy: PatchPolicy::Dmx,
            notes: String::new(),
            photograph_asset: None,
            stage_icon_asset: None,
            model_asset: None,
            model_units: ModelUnits::Auto,
            physical: ProfilePhysicalProperties::default(),
            modes: vec![FixtureMode {
                id: mode_id,
                name: "Default".into(),
                notes: String::new(),
                splits: vec![FixtureSplit {
                    number: 1,
                    footprint: 1,
                }],
                heads: vec![FixtureHead {
                    id: head_id,
                    name: "Main".into(),
                    master_shared: true,
                }],
                channels: Vec::new(),
                color_systems: Vec::new(),
                control_actions: Vec::new(),
                geometry: GeometryGraph::template(GeometryTemplate::Fixed, &[head_id]),
            }],
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: SignalLossPolicy::HoldLast,
            reserved_source: None,
        }
    }

    pub fn mode(&self, id: Uuid) -> Option<&FixtureMode> {
        self.modes.iter().find(|mode| mode.id == id)
    }
}
