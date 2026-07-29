//! Translation from runtime/domain models to the v2 connection-lifecycle wire contract.

use super::*;
use light_wire::v2::runtime as wire;

pub(super) fn user(user: DeskUser) -> wire::RuntimeDeskUser {
    wire::RuntimeDeskUser {
        id: user.id.0,
        name: user.name,
        enabled: user.enabled,
    }
}

pub(super) fn desk(desk: ControlDesk) -> wire::RuntimeControlDesk {
    wire::RuntimeControlDesk {
        id: desk.id,
        name: desk.name,
        osc_alias: desk.osc_alias,
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
    descriptor: &light_core::AttributeDescriptor,
) -> wire::RuntimeAttributeDescriptor {
    wire::RuntimeAttributeDescriptor {
        id: descriptor.id.into(),
        label: descriptor.label.into(),
        family: attribute_class(descriptor.family).into(),
        value_type: attribute_value_type(descriptor.value_type).into(),
        default_unit: descriptor.default_unit.map(str::to_owned),
        display_unit: descriptor.display_unit.map(str::to_owned),
        physical_unit: descriptor.physical_unit.map(str::to_owned),
        normalized_min: descriptor.normalized_bounds.map(|bounds| bounds.min),
        normalized_max: descriptor.normalized_bounds.map(|bounds| bounds.max),
        domain_min: descriptor.domain_bounds.map(|bounds| bounds.min),
        domain_max: descriptor.domain_bounds.map(|bounds| bounds.max),
        cyclic: descriptor.cyclic,
        recordable: descriptor.recordable,
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
        owner_user_id: state.owner_user_id.map(|id| id.0),
        owner_user_name: state.owner_user_name,
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

const fn attribute_class(value: light_core::AttributeClass) -> &'static str {
    match value {
        light_core::AttributeClass::Intensity => "intensity",
        light_core::AttributeClass::Position => "position",
        light_core::AttributeClass::Color => "color",
        light_core::AttributeClass::Beam => "beam",
        light_core::AttributeClass::Shapers => "shapers",
        light_core::AttributeClass::Focus => "focus",
        light_core::AttributeClass::Control => "control",
        light_core::AttributeClass::Media => "media",
        light_core::AttributeClass::Custom => "custom",
    }
}

const fn attribute_value_type(value: light_core::AttributeValueType) -> &'static str {
    match value {
        light_core::AttributeValueType::Continuous => "continuous",
        light_core::AttributeValueType::Color => "color",
        light_core::AttributeValueType::Indexed => "indexed",
        light_core::AttributeValueType::Control => "control",
    }
}

const fn highlight_mode(value: HighlightMode) -> &'static str {
    match value {
        HighlightMode::Selection => "selection",
        HighlightMode::Step => "step",
    }
}
