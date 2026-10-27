//! Translation from runtime/domain models to the v2 connection-lifecycle wire contract.

use super::*;
use light_wire::v2::attribute_configuration as attribute_wire;
use light_wire::v2::runtime as wire;

#[cfg(test)]
mod fine_histogram_tests {
    use super::*;

    #[tokio::test]
    async fn wire_omits_empty_fine_data_and_publishes_complete_scheduler_counts() {
        let health = OutputHealth::default();
        let empty = output_health(health.clone(), light_output::ChangeLeadSnapshot::default());
        assert!(
            serde_json::to_value(empty)
                .unwrap()
                .get("tick_duration_fine_histogram")
                .is_none()
        );
        let shared = std::sync::Arc::new(std::sync::Mutex::new(health));
        let cancel = tokio_util::sync::CancellationToken::new();
        let stop = cancel.clone();
        light_output::run_scheduler(44, cancel, std::sync::Arc::clone(&shared), move || {
            stop.cancel();
            async { Ok(0) }
        })
        .await;
        let snapshot = shared.lock().unwrap().clone();
        let expected_coarse = snapshot.tick_duration_bucket_counts.to_vec();
        let wire = output_health(snapshot, light_output::ChangeLeadSnapshot::default());
        assert_eq!(wire.tick_duration_bucket_counts, expected_coarse);
        let fine = wire.tick_duration_fine_histogram.as_ref().unwrap();
        assert_eq!(fine.schema_version, 1);
        assert_eq!(
            fine.bounds_micros,
            light_output::OUTPUT_FINE_TICK_DURATION_BUCKET_BOUNDS_MICROS
        );
        assert_eq!(fine.sample_count, 1);
        assert_eq!(fine.counts.iter().sum::<u64>(), fine.sample_count);
        let json = serde_json::to_value(wire).unwrap();
        assert_eq!(json["tick_duration_fine_histogram"]["sample_count"], 1);
        assert_eq!(
            json["tick_duration_fine_histogram"]["counts"]
                .as_array()
                .unwrap()
                .len(),
            133
        );
    }
}

pub(super) fn desk(desk: ControlDesk) -> wire::RuntimeControlDesk {
    wire::RuntimeControlDesk {
        show_compact_encoders: desk.show_compact_encoders,
        hardware_led_brightness: desk.hardware_led_brightness,
        hardware_gooseneck_brightness: desk.hardware_gooseneck_brightness,
        hardware_gooseneck_color: desk.hardware_gooseneck_color,
        id: desk.id,
        name: desk.name,
        columns: desk.columns,
        rows: desk.rows,
        buttons: desk.buttons,
        surface_id: desk.surface_id,
    }
}

pub(super) fn show(show: ShowEntry) -> wire::RuntimeShowEntry {
    wire::RuntimeShowEntry {
        is_base_show: show.is_base_show,
        id: show.id.0,
        name: show.name,
        path: show.path,
        revision: show.revision,
        updated_at: show.updated_at,
        created_at: show.created_at,
        last_loaded_at: show.last_loaded_at,
        revision_copy: show
            .revision_copy
            .map(|source| wire::RuntimeRevisionCopySource {
                show_id: source.show_id.0,
                show_name: source.show_name,
                revision: source.revision,
                revision_name: source.revision_name,
                copied_at: source.copied_at,
            }),
    }
}

pub(super) fn output_health(
    health: OutputHealth,
    change_lead: light_output::ChangeLeadSnapshot,
) -> wire::RuntimeOutputHealth {
    let tick_duration_fine_histogram =
        health
            .complete_fine_tick_duration_counts()
            .map(|(counts, sample_count)| wire::OutputTickDurationHistogram {
                schema_version: 1,
                bounds_micros: light_output::OUTPUT_FINE_TICK_DURATION_BUCKET_BOUNDS_MICROS
                    .to_vec(),
                counts: counts.to_vec(),
                sample_count,
            });
    wire::RuntimeOutputHealth {
        frames_sent: health.frames_sent,
        packets_sent: health.packets_sent,
        send_errors: health.send_errors,
        deadline_misses: health.deadline_misses,
        maximum_lateness_micros: health.maximum_lateness_micros,
        frame_hz: health.frame_hz,
        last_tick_micros: health.last_tick_micros,
        maximum_tick_micros: health.maximum_tick_micros,
        tick_duration_bucket_bounds_micros: light_output::OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS
            .to_vec(),
        tick_duration_bucket_counts: health.tick_duration_bucket_counts.to_vec(),
        tick_duration_fine_histogram,
        scheduler_utilization: health.scheduler_utilization,
        recent_window_seconds: light_output::OUTPUT_RECENT_WINDOW.as_secs() as u32,
        recent_frame_hz_minimum: health.recent_frame_hz_minimum,
        recent_frame_hz_maximum: health.recent_frame_hz_maximum,
        recent_frame_hz_average: health.recent_frame_hz_average,
        recent_frame_rate_bucket_bounds_hz: light_output::OUTPUT_FRAME_RATE_BUCKET_BOUNDS_HZ
            .to_vec(),
        recent_frame_rate_bucket_counts: health.recent_frame_rate_bucket_counts.to_vec(),
        recent_send_errors: health.recent_send_errors,
        frame_rate_band_bounds_hz: light_output::OUTPUT_FRAME_RATE_BAND_BOUNDS_HZ.to_vec(),
        frame_rate_band_counts: health.frame_rate_band_counts.clone(),
        change_lead: change_lead_time(change_lead),
    }
}

pub(super) fn change_lead_time(
    change_lead: light_output::ChangeLeadSnapshot,
) -> wire::RuntimeChangeLeadTime {
    wire::RuntimeChangeLeadTime {
        maximum_micros: change_lead.maximum_micros,
        recent_maximum_micros: change_lead.recent_maximum_micros,
        last_micros: change_lead.last_micros,
        recent_window_seconds: light_output::CHANGE_LEAD_RECENT_WINDOW_SECONDS,
        samples: change_lead.samples,
        excluded: change_lead.excluded,
        plausible_limit_micros: light_output::CHANGE_LEAD_PLAUSIBLE_LIMIT_MICROS,
    }
}

pub(super) fn attribute(
    descriptor: attribute_wire::ConfiguredAttributeDescriptor,
) -> wire::RuntimeAttributeDescriptor {
    wire::RuntimeAttributeDescriptor {
        id: descriptor.id,
        label: descriptor.label,
        family: attribute_encoder_group(descriptor.encoder_group).into(),
        value_type: attribute_value_type(descriptor.value_type).into(),
        default_unit: descriptor.display_unit.clone(),
        display_unit: descriptor.display_unit,
        physical_unit: descriptor.physical_unit,
        normalized_min: descriptor.normalized_min,
        normalized_max: descriptor.normalized_max,
        domain_min: descriptor.domain_min,
        domain_max: descriptor.domain_max,
        cyclic: descriptor.cyclic,
        recordable: descriptor.recordable,
        encoder_group: descriptor.encoder_group,
        encoder_page: descriptor.encoder_page,
        encoder_slot: descriptor.encoder_slot,
        built_in: descriptor.built_in,
        retired: descriptor.retired,
        activation_group_id: descriptor.activation_group_id,
        push_turn_of: descriptor.push_turn_of,
    }
}

fn attribute_encoder_group(group: attribute_wire::AttributeEncoderGroup) -> &'static str {
    match group {
        attribute_wire::AttributeEncoderGroup::Intensity => "intensity",
        attribute_wire::AttributeEncoderGroup::Color => "color",
        attribute_wire::AttributeEncoderGroup::Position => "position",
        attribute_wire::AttributeEncoderGroup::Beam => "beam",
        attribute_wire::AttributeEncoderGroup::Shapers => "shapers",
        attribute_wire::AttributeEncoderGroup::Focus => "focus",
        attribute_wire::AttributeEncoderGroup::Control => "control",
        attribute_wire::AttributeEncoderGroup::Media => "media",
    }
}

pub(super) fn highlight(state: HighlightState) -> wire::RuntimeHighlightState {
    wire::RuntimeHighlightState {
        active: state.active,
        mode: highlight_mode(state.mode).into(),
        output_enabled: state.output_enabled,
        capture_only: state.capture_only,
        remembered: state
            .remembered
            .into_iter()
            .map(highlight_fixture)
            .collect(),
        active_index: state.active_index,
        active_fixture: state.active_fixture.map(highlight_fixture),
        can_previous: state.can_previous,
        can_next: state.can_next,
        message: state.message,
    }
}

fn highlight_fixture(fixture: HighlightFixture) -> wire::RuntimeHighlightFixture {
    wire::RuntimeHighlightFixture {
        fixture_id: fixture.fixture_id.0,
        name: fixture.name,
        number: fixture.number,
    }
}

const fn attribute_value_type(value: attribute_wire::AttributeValueType) -> &'static str {
    match value {
        attribute_wire::AttributeValueType::Continuous => "continuous",
        attribute_wire::AttributeValueType::Color => "color",
        attribute_wire::AttributeValueType::Position => "position",
        attribute_wire::AttributeValueType::Indexed => "indexed",
        attribute_wire::AttributeValueType::Control => "control",
    }
}

const fn highlight_mode(value: HighlightMode) -> &'static str {
    match value {
        HighlightMode::Selection => "selection",
        HighlightMode::Step => "step",
    }
}
