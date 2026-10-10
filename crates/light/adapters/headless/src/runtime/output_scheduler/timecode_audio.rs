use super::*;

fn audio_player_uses_media_attributes(fixture: &light_fixture::PatchedFixture) -> bool {
    fixture
        .definition
        .heads
        .iter()
        .flat_map(|head| head.parameters.iter())
        .any(|parameter| *parameter.attribute.0 == *"media.play_mode")
}

/// Paused holds the voice; playing selects Loop when the track repeats and Once - Hold otherwise.
fn timecode_play_mode(transport: u32, repeat: bool) -> u32 {
    match (transport, repeat) {
        (64, _) => 236,
        (_, true) => 0,
        (_, false) => 60,
    }
}

pub(super) fn timecode_audio_contributions(
    timecodes: &light_application::timeline::TimecodeRuntimeService,
    fixtures: &[light_fixture::PatchedFixture],
    changed_at: chrono::DateTime<chrono::Utc>,
) -> ContributionBatch {
    let rate = timecodes.frame_rate();
    let mut order = 0_u64;
    ContributionBatch::new(timecodes.snapshots().into_iter().flat_map(|snapshot| {
        let transport = match snapshot.transport {
            TimecodeTransportState::Stopped => return Vec::new(),
            TimecodeTransportState::Paused => 64_u32,
            TimecodeTransportState::Playing => 128_u32,
        };
        snapshot
            .reconstructed
            .audio_players
            .into_iter()
            .flat_map(|player| {
                let fixture = fixtures
                    .iter()
                    .find(|fixture| fixture.fixture_id == player.fixture_id);
                let fixture_id = fixture
                    .and_then(|fixture| fixture.logical_heads.first())
                    .map_or(player.fixture_id, |head| head.fixture_id);
                let canonical = fixture.is_some_and(audio_player_uses_media_attributes);
                let cursor_millis = u32::try_from(
                    u128::from(player.cursor_frame.0)
                        .saturating_mul(u128::from(rate.denominator()))
                        .saturating_mul(1_000)
                        / u128::from(rate.numerator()),
                )
                .unwrap_or(u32::MAX);
                let mut samples = vec![
                    (
                        if canonical {
                            "media.folder"
                        } else {
                            "audio.folder"
                        },
                        AttributeValue::RawDmxExact(u32::from(player.folder)),
                    ),
                    (
                        if canonical {
                            "media.file"
                        } else {
                            "audio.file"
                        },
                        AttributeValue::RawDmxExact(u32::from(player.file)),
                    ),
                    (
                        if canonical { "volume" } else { "audio.volume" },
                        AttributeValue::Normalized(player.volume),
                    ),
                    (
                        "audio.cursor_millis",
                        AttributeValue::RawDmxExact(cursor_millis),
                    ),
                ];
                if canonical {
                    // Play mode carries transport and repeat together on the TL-367 personality.
                    samples.push((
                        "media.play_mode",
                        AttributeValue::RawDmxExact(timecode_play_mode(transport, player.repeat)),
                    ));
                } else {
                    samples.push(("audio.transport", AttributeValue::RawDmxExact(transport)));
                    samples.push((
                        "audio.repeat",
                        AttributeValue::RawDmxExact(if player.repeat { 255 } else { 0 }),
                    ));
                }
                samples
                    .into_iter()
                    .map(|(attribute, value)| {
                        order = order.saturating_add(1);
                        ContributionSample::independent(TimedValue {
                            fixture_id,
                            attribute: AttributeKey(attribute.into()),
                            value,
                            priority: 75,
                            changed_at,
                            programmer_order: order,
                            merge_mode: MergeMode::Ltp,
                            fade: false,
                            fade_millis: None,
                            delay_millis: None,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }))
}
