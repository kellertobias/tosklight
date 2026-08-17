/// Each patched logical head with the attributes it actually owns, so a surface can grey out
/// controls this personality does not carry instead of inventing them.
pub(super) fn media_layers(fixture: &light_fixture::PatchedFixture) -> Vec<serde_json::Value> {
    fixture
        .logical_heads
        .iter()
        .map(|patched| {
            serde_json::json!({
                "fixture_id": patched.fixture_id,
                "head_index": patched.head_index,
                "attributes": head_attributes(fixture, |head| head.index == patched.head_index),
            })
        })
        .collect()
}

/// Attributes of the shared master head, which the desk addresses through the parent fixture.
pub(super) fn master_head_attributes(fixture: &light_fixture::PatchedFixture) -> Vec<String> {
    let shared = head_attributes(fixture, |head| head.shared);
    if shared.is_empty() {
        head_attributes(fixture, |_| true)
    } else {
        shared
    }
}

fn head_attributes(
    fixture: &light_fixture::PatchedFixture,
    keep: impl Fn(&light_fixture::LogicalHead) -> bool,
) -> Vec<String> {
    let mut attributes = fixture
        .definition
        .heads
        .iter()
        .filter(|head| keep(head))
        .flat_map(|head| head.parameters.iter())
        .map(|parameter| parameter.attribute.0.clone())
        .collect::<Vec<_>>();
    attributes.sort();
    attributes.dedup();
    attributes
}

pub(super) fn is_media_server_fixture(fixture: &light_fixture::PatchedFixture) -> bool {
    fixture.definition.device_type.trim() == "media_server"
}

/// The desk-local Internal Audio Player is a media source without DMX address or CITP endpoint.
pub(super) fn is_audio_player_fixture(fixture: &light_fixture::PatchedFixture) -> bool {
    fixture.definition.device_type.trim() == "audio_player"
}

#[cfg(test)]
mod media_server_fixture_tests {
    use super::{is_audio_player_fixture, is_media_server_fixture};

    fn fixture(
        fixture_type: &str,
        patch_policy: light_fixture::PatchPolicy,
        citp: bool,
    ) -> light_fixture::PatchedFixture {
        let mut profile = light_fixture::FixtureProfile::blank();
        profile.manufacturer = "Test".into();
        profile.name = "Media role".into();
        profile.fixture_type = fixture_type.into();
        profile.patch_policy = patch_policy;
        if patch_policy == light_fixture::PatchPolicy::Internal {
            profile.modes[0].splits[0].footprint = 0;
        }
        if citp {
            profile.direct_control_protocols = vec![light_fixture::DirectControlProtocol::Citp];
        }
        let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
        serde_json::from_value(serde_json::json!({
            "fixture_id": light_core::FixtureId::new(),
            "definition": definition
        }))
        .unwrap()
    }

    #[test]
    fn eligibility_is_fixture_type_driven_for_zero_one_and_multiple_servers() {
        let ordinary = fixture("wash", light_fixture::PatchPolicy::Dmx, true);
        let external = fixture("media_server", light_fixture::PatchPolicy::Dmx, false);
        let citp = fixture("media_server", light_fixture::PatchPolicy::Dmx, true);
        let internal = fixture("media_server", light_fixture::PatchPolicy::Internal, false);

        assert!(!is_media_server_fixture(&ordinary));
        assert!(is_media_server_fixture(&external));
        assert!(is_media_server_fixture(&citp));
        assert!(is_media_server_fixture(&internal));
        assert_eq!(
            [ordinary.clone()]
                .iter()
                .filter(|candidate| is_media_server_fixture(candidate))
                .count(),
            0
        );
        assert_eq!(
            [external.clone()]
                .iter()
                .filter(|candidate| is_media_server_fixture(candidate))
                .count(),
            1
        );
        assert_eq!(
            [external, citp, internal]
                .iter()
                .filter(|candidate| is_media_server_fixture(candidate))
                .count(),
            3
        );
    }

    #[test]
    fn every_patched_audio_player_is_a_media_pane_source() {
        let player = fixture("audio_player", light_fixture::PatchPolicy::Internal, false);
        let second = fixture("audio_player", light_fixture::PatchPolicy::Internal, false);
        let ordinary = fixture("wash", light_fixture::PatchPolicy::Dmx, true);
        assert!(is_audio_player_fixture(&player));
        assert!(!is_media_server_fixture(&player));
        assert!(!is_audio_player_fixture(&ordinary));
        assert_eq!(
            [player, second, ordinary]
                .iter()
                .filter(|candidate| is_audio_player_fixture(candidate))
                .count(),
            2
        );
    }
}
