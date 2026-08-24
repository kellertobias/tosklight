//! Translation from runtime/domain models to the v2 connection-lifecycle wire contract.

use super::*;
use light_wire::v2::attribute_configuration as attribute_wire;
use light_wire::v2::runtime as wire;

pub(super) fn desk(desk: ControlDesk) -> wire::RuntimeControlDesk {
    wire::RuntimeControlDesk {
        id: desk.id,
        name: desk.name,
        columns: desk.columns,
        rows: desk.rows,
        buttons: desk.buttons,
        playback_layout: desk
            .playback_layout
            .map(|layout| wire::RuntimePlaybackSurfaceLayout {
                playbacks_per_row: layout.playbacks_per_row,
                rows: layout
                    .rows
                    .into_iter()
                    .map(|row| wire::RuntimePlaybackSurfaceRow {
                        first_playback_slot: row.first_playback_slot,
                        has_fader: row.has_fader,
                        button_count: row.button_count,
                    })
                    .collect(),
            }),
    }
}

pub(super) fn show(show: ShowEntry) -> wire::RuntimeShowEntry {
    wire::RuntimeShowEntry {
        id: show.id.0,
        name: show.name,
        path: show.path,
        revision: show.revision,
        updated_at: show.updated_at,
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

pub(super) fn output_health(health: OutputHealth) -> wire::RuntimeOutputHealth {
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
        scheduler_utilization: health.scheduler_utilization,
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
