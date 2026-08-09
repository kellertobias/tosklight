use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_affecting_events_are_the_configuration_ones_only() {
        assert!(scene_affecting("show_patch_changed"));
        assert!(scene_affecting("active_show_changed"));
        // Live output must never arrive through the event subscription.
        assert!(!scene_affecting("output_frame"));
        assert!(!scene_affecting("programmer_changed"));
        assert!(!scene_affecting("playback_changed"));
    }

    /// An edit to the loaded show is applied in place; a different show is staged whole.
    #[test]
    fn only_a_change_of_show_costs_the_displayed_values() {
        assert!(replaces_the_show("show_library_changed"));
        assert!(replaces_the_show("active_show_changed"));
        assert!(!replaces_the_show("show_patch_changed"));
        assert!(!replaces_the_show("show_objects_changed"));
        assert!(!replaces_the_show("output_route_changed"));
        assert!(!replaces_the_show("fixture_library_changed"));
        for kind in [
            "show_patch_changed",
            // What a desk actually publishes when a show object is written.
            "show_objects_changed",
            // What the planning window publishes for the same thing.
            "show_object_changed",
            "output_route_changed",
            "fixture_library_changed",
        ] {
            assert!(
                scene_affecting(kind),
                "{kind} must still reach the delta path"
            );
        }
    }

    /// The view arrives on the desk's own configuration capability, not on a stream of its own.
    #[test]
    fn the_view_follows_desk_configuration_and_nothing_else() {
        assert!(view_affecting("server_configuration_changed"));
        assert!(view_affecting("visualizer_view_changed"));
        assert!(!view_affecting("show_patch_changed"));
        assert!(!view_affecting("output_frame"));
    }
}

#[cfg(test)]
mod network_rule_tests {
    use super::*;

    /// The rule the embedded pane turns off, stated where it can be checked.
    ///
    /// A renderer on a network waits for real packets, because that is what a lighting rig sends.
    /// One inside the desk's own window is handed the same numbers and must not bind sockets as
    /// well: two processes on one machine competing for the same multicast groups, to learn
    /// something one of them already knows, is a way to lose packets rather than gain values.
    #[test]
    fn only_a_renderer_on_its_own_waits_for_packets() {
        let on_a_network = DeskProvider::start(DeskConnection::default(), Instant::now());
        assert!(
            on_a_network.listens_on_the_network(),
            "a renderer on its own keeps the two-plane rule"
        );

        let in_the_desk = DeskProvider::start(
            DeskConnection {
                values_from_desk_output: true,
                ..DeskConnection::default()
            },
            Instant::now(),
        );
        assert!(
            !in_the_desk.listens_on_the_network(),
            "one in the desk's window is handed its values instead"
        );
    }
}

#[cfg(test)]
mod preview_precedence {
    use super::*;
    use std::collections::BTreeSet;

    fn preview(universes: &[u16]) -> crate::wire::PreviewSnapshot {
        crate::wire::PreviewSnapshot {
            revision: 1,
            universes: universes
                .iter()
                .map(|universe| crate::wire::PreviewUniverse {
                    universe: *universe,
                    slots: vec![0; viz_dmx::DMX_SLOTS],
                })
                .collect(),
        }
    }

    #[test]
    fn the_editor_drives_every_universe_no_source_has_delivered() {
        let driven = editor_driven(&preview(&[1, 2, 3]), &BTreeSet::new());
        assert_eq!(driven, BTreeSet::from([1, 2, 3]));
    }

    /// A real source taking one universe takes only that one; the rest keep the editor's values.
    #[test]
    fn a_real_source_takes_its_own_universe_and_no_other() {
        let driven = editor_driven(&preview(&[1, 2, 3]), &BTreeSet::from([2]));
        assert_eq!(driven, BTreeSet::from([1, 3]));
    }

    /// Losing a source afterwards holds the last received values rather than handing the universe
    /// back to the editor. `real` is what has *ever* arrived, so a universe never returns.
    #[test]
    fn a_universe_that_has_had_dmx_never_returns_to_the_editor() {
        let ever_received = BTreeSet::from([2]);
        // The source has stopped; nothing about that changes what has already been delivered.
        let driven = editor_driven(&preview(&[1, 2, 3]), &ever_received);
        assert!(
            !driven.contains(&2),
            "universe 2 went back to the editor after its source was lost"
        );
    }

    /// A desk serves no preview plane at all, so nothing is ever editor-driven there.
    #[test]
    fn a_desk_has_no_editor_driven_universes() {
        let driven = editor_driven(
            &crate::wire::PreviewSnapshot::default(),
            &BTreeSet::from([1, 2]),
        );
        assert!(driven.is_empty());
    }
}
