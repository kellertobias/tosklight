use super::*;
use media_application::configuration::MonitorSelector;

fn monitor_output() -> OutputConfiguration {
    let mut output = OutputConfiguration::new("Main");
    output.target = OutputTarget::Monitor {
        monitor: MonitorSelector::Index(0),
        fullscreen: false,
    };
    output
}

#[test]
fn an_all_off_screen_configuration_asks_for_no_output_window() {
    let mut output = OutputConfiguration::new("Main");
    output.target = OutputTarget::OffScreen;
    let configuration = MediaConfiguration {
        outputs: vec![output],
        ..Default::default()
    };
    assert!(matches!(
        configuration.outputs[0].target,
        OutputTarget::OffScreen
    ));
    assert!(!needs_a_window(&configuration));
}

#[test]
fn a_monitor_bound_output_asks_for_an_output_window() {
    let configuration = MediaConfiguration {
        outputs: vec![monitor_output()],
        ..Default::default()
    };
    assert!(needs_a_window(&configuration));
}

#[test]
fn arrow_navigation_chooses_the_nearest_display_in_that_direction() {
    let current = DisplayRectangle {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let displays = [
        current,
        DisplayRectangle {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
        },
        DisplayRectangle {
            x: 1800,
            y: 1440,
            width: 1920,
            height: 1080,
        },
        DisplayRectangle {
            x: -1280,
            y: 0,
            width: 1280,
            height: 1024,
        },
    ];

    assert_eq!(
        nearest_display(current, &displays, DisplayDirection::Right),
        Some(1)
    );
    assert_eq!(
        nearest_display(current, &displays, DisplayDirection::Left),
        Some(3)
    );
    assert_eq!(
        nearest_display(current, &displays, DisplayDirection::Down),
        Some(2)
    );
    assert_eq!(
        nearest_display(current, &displays, DisplayDirection::Up),
        None
    );
}

#[test]
fn arrow_navigation_does_nothing_at_the_edge_of_the_desktop() {
    let current = DisplayRectangle {
        x: 1920,
        y: 0,
        width: 1920,
        height: 1080,
    };
    let displays = [
        DisplayRectangle {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        },
        current,
    ];
    assert_eq!(
        nearest_display(current, &displays, DisplayDirection::Right),
        None
    );
}

#[test]
fn fullscreen_commands_require_exactly_control_and_shift() {
    assert!(windows_command_chord(
        ModifiersState::CONTROL | ModifiersState::SHIFT
    ));
    assert!(!windows_command_chord(ModifiersState::CONTROL));
    assert!(!windows_command_chord(ModifiersState::SHIFT));
    assert!(!windows_command_chord(
        ModifiersState::CONTROL | ModifiersState::SHIFT | ModifiersState::ALT
    ));
}

#[test]
fn an_unconfigured_first_run_asks_for_its_visible_output_window() {
    assert!(needs_a_window(&MediaConfiguration::default()));
}

#[test]
fn the_output_window_has_the_pixel_application_icon() {
    assert!(application_icon().is_some());
}

#[test]
fn a_disabled_monitor_output_does_not_open_a_window() {
    let mut output = monitor_output();
    output.enabled = false;
    let configuration = MediaConfiguration {
        outputs: vec![output],
        ..Default::default()
    };
    assert!(!needs_a_window(&configuration));
}

#[test]
fn presentation_worker_waits_for_the_earliest_fixed_deadline() {
    assert_eq!(
        presentation_worker_wait([
            std::time::Duration::from_millis(33),
            std::time::Duration::from_millis(16),
        ]),
        Some(std::time::Duration::from_millis(16))
    );
    assert_eq!(presentation_worker_wait([std::time::Duration::ZERO]), None);
    assert_eq!(presentation_worker_wait([]), None);
}

#[test]
fn an_unlocked_output_keeps_a_mixed_worker_running_immediately() {
    assert_eq!(
        presentation_worker_wait([
            std::time::Duration::from_millis(16),
            std::time::Duration::ZERO,
            std::time::Duration::from_millis(33),
        ]),
        None
    );
}

#[test]
fn what_the_renderer_saw_reaches_the_authoritative_state() {
    let id = media_domain::OutputId::new();
    let state = MediaState::with_outputs(vec![media_domain::OutputState::new(
        id,
        media_domain::LayerPersonality::TwoLayers,
    )]);
    let failure = media_domain::SourceStatus::Failed {
        failure: media_domain::SourceFailure::MissingFile,
    };

    let next = with_reports(&state, &[(id, 0, failure)], Timestamp::from_millis(0))
        .expect("a new status is a change");
    assert_eq!(next.output(id).unwrap().layers[0].source_status, failure);
    assert_eq!(
        next.output(id).unwrap().layers[1].source_status,
        media_domain::SourceStatus::Unselected,
        "one layer's failure is not another's"
    );

    assert!(
        with_reports(&next, &[(id, 0, failure)], Timestamp::from_millis(16)).is_none(),
        "reporting the same status again publishes nothing"
    );
    assert!(
        with_reports(
            &state,
            &[(media_domain::OutputId::new(), 0, failure)],
            Timestamp::from_millis(0)
        )
        .is_none(),
        "a report for an output that is not here changes nothing"
    );
}
