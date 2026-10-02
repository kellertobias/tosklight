//! TL-601: accepted optics continuity against fresh authored native edits and changed physical
//! response with stable native identities. Every resolution is verified through the encoded DMX
//! bytes and an independently compiled forward model (`Resolved::verify`).
use super::*;
use light_fixture::{ChannelFunctionBehavior, FixtureChannel};

fn function_ids(profile: &FixtureProfile, channel: usize) -> Vec<Uuid> {
    profile.modes[0].channels[channel]
        .functions
        .iter()
        .map(|f| f.id)
        .collect()
}

impl Rig {
    /// A verified resolution against `previous` (this lane's last accepted continuity).
    fn after(
        &self,
        owner: ProgrammingOwner,
        value: &AttributeValue,
        previous: &OpticsContinuity,
    ) -> Resolved {
        let resolved = self
            .resolve_with(owner, value, Some(previous), RenderOptions::default())
            .unwrap();
        resolved.verify(value, &self.profile.borrow());
        resolved
    }
}

#[test]
fn a_fresh_native_function_edit_is_respected_over_accepted_zoom_continuity() {
    let profile = multi_function_zoom();
    let ids = function_ids(&profile, 1);
    let rig = Rig::new(&profile);
    // Authored command: the wide (ascending 20° → 60°) function.
    rig.set_raw("zoom", 200);
    rig.set("focus", 0.4);
    let accepted = rig.zoom(&field(50.));
    assert_eq!(accepted.result.quality.function_id, Some(ids[2]));
    let zoom_continuity = accepted.result.continuity;
    let focus_accepted = rig.focus(&focus(0.25));
    let focus_continuity = focus_accepted.result.continuity;

    // Unchanged command: continuity is legitimately reused. An unfittable request (the other
    // convention) holds the last accepted raw, not the baseline.
    let held = rig.after(
        ProgrammingOwner::Zoom,
        &zoom(30., OpeningConvention::Beam),
        &zoom_continuity,
    );
    assert_eq!(
        held.result.quality.status,
        OpticsFitStatus::ConventionMismatch
    );
    assert!(held.result.quality.held);
    assert_eq!(
        held.raw(),
        accepted.raw(),
        "stable command keeps continuity"
    );

    // A real authored edit to the reversed narrow function (40° → 10° on 0..=99).
    rig.set_raw("zoom", 50);
    let edited = rig.after(ProgrammingOwner::Zoom, &field(30.), &zoom_continuity);
    assert_eq!(
        edited.result.quality.function_id,
        Some(ids[0]),
        "the edited function is used, the accepted wide raw is not replayed"
    );
    assert_eq!(edited.result.quality.status, OpticsFitStatus::Fitted);
    assert!((0..=99).contains(&edited.raw()));
    assert_eq!(edited.raw(), 33, "30° on 40° → 10° over 0..=99");
    assert!((edited.achieved() - 30.).abs() <= 0.5 * 30. / 99. + 1e-9);
    assert_eq!(edited.requested(), 30., "the request is unchanged");
    // Held output follows the edited command too.
    let held = rig.after(
        ProgrammingOwner::Zoom,
        &zoom(30., OpeningConvention::Beam),
        &zoom_continuity,
    );
    assert!(held.result.quality.held);
    assert_eq!(
        held.raw(),
        50,
        "held at the current command, not the old raw"
    );
    assert_eq!(held.result.quality.function_id, Some(ids[0]));
    // An edit into the macro is passive Ambiguous at the commanded raw: no reinstalled output.
    rig.set_raw("zoom", 110);
    let ambiguous = rig.after(ProgrammingOwner::Zoom, &field(30.), &zoom_continuity);
    assert_eq!(ambiguous.result.quality.status, OpticsFitStatus::Ambiguous);
    assert!(ambiguous.result.quality.held);
    assert_eq!(ambiguous.raw(), 110);
    assert_eq!(ambiguous.result.achieved, None, "unknown, not zero");

    // The new continuity chains from the edited command: stable again on the next frame.
    rig.set_raw("zoom", 50);
    let first = rig.after(ProgrammingOwner::Zoom, &field(30.), &zoom_continuity);
    let next = rig.after(
        ProgrammingOwner::Zoom,
        &zoom(30., OpeningConvention::Beam),
        &first.result.continuity,
    );
    assert_eq!(next.raw(), first.raw(), "the edited accepted raw is reused");

    // Focus is independent: its own continuity and write are untouched by Zoom edits.
    let focused = rig.after(ProgrammingOwner::Focus, &focus(0.25), &focus_continuity);
    assert_eq!(focused.write(), focus_accepted.write());
    assert_eq!(focused.result.continuity, focus_continuity);
}

/// The narrow function of [`multi_function_zoom`] with a changed physical response; every
/// channel and function UUID is unchanged.
fn changed_responses(profile: &FixtureProfile) -> Vec<(&'static str, FixtureProfile)> {
    let edit = |name, change: &dyn Fn(&mut FixtureChannel)| {
        let mut changed = profile.clone();
        let mode = &mut changed.modes[0];
        change(&mut mode.channels[1]);
        if mode.channels[1].resolution != ChannelResolution::U8 {
            mode.splits[0].footprint += 1;
            let slot = 2 + 1;
            mode.channels[1].secondary_slots = vec![slot];
        }
        changed.validate().unwrap();
        (name, changed)
    };
    vec![
        edit("convention", &|channel| {
            let mapping = channel.functions[0].physical_mapping.as_mut().unwrap();
            mapping.opening_convention = Some(OpeningConvention::Beam);
        }),
        edit("calibration", &|channel| {
            channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
                physical_min: 35.,
                physical_max: 10.,
                unit: Some("deg".into()),
            };
        }),
        edit("function range", &|channel| {
            channel.functions[0].dmx_to = 94;
            channel.functions[1].dmx_from = 95;
        }),
        edit("mode resolution", &|channel| {
            channel.resolution = ChannelResolution::U16;
        }),
    ]
}

#[test]
fn changed_zoom_response_with_stable_native_ids_invalidates_continuity() {
    let profile = multi_function_zoom();
    let rig = Rig::new(&profile);
    rig.set_raw("zoom", 90);
    let accepted = rig.zoom(&field(30.));
    assert_eq!(accepted.raw(), 33);
    let continuity = accepted.result.continuity;
    // 0° is never fittable: the held raw shows which output the continuity reinstalls.
    let closed = field(0.);

    // An unrelated show edit: the same response in a newly installed fixture list.
    rig.install(&profile);
    let held = rig.after(ProgrammingOwner::Zoom, &closed, &continuity);
    assert_eq!(held.result.quality.status, OpticsFitStatus::InvalidRequest);
    assert_eq!(held.raw(), 33, "unchanged response keeps continuity");
    assert_eq!(held.descriptor.response, accepted.descriptor.response);

    for (name, changed) in changed_responses(&profile) {
        assert_eq!(
            changed.modes[0].channels[1].id, profile.modes[0].channels[1].id,
            "{name}: native identities are stable"
        );
        assert_eq!(function_ids(&changed, 1), function_ids(&profile, 1));
        rig.install(&changed);
        let held = rig.after(ProgrammingOwner::Zoom, &closed, &continuity);
        assert!(held.result.quality.held, "{name}");
        assert_ne!(
            held.descriptor.response, accepted.descriptor.response,
            "{name}: the response digest changes"
        );
        assert_eq!(
            held.raw(),
            90,
            "{name}: the old raw is not reinstalled under another response"
        );
        // A fittable request is solved by the new response from the current command.
        let fitted = rig.after(ProgrammingOwner::Zoom, &field(20.), &continuity);
        assert_eq!(fitted.requested(), 20., "{name}");
        if name == "convention" {
            // The narrow function is now Beam: checked, never converted, held at the command.
            assert_eq!(
                fitted.result.quality.status,
                OpticsFitStatus::ConventionMismatch
            );
            assert_eq!(fitted.raw(), 90);
        } else {
            assert_eq!(
                fitted.result.quality.status,
                OpticsFitStatus::Fitted,
                "{name}"
            );
            assert_eq!(
                fitted.write().function_id,
                Some(function_ids(&profile, 1)[0]),
                "{name}"
            );
            assert!((fitted.achieved() - 20.).abs() < 0.5, "{name}");
        }
        // The proposed continuity under the new response chains normally.
        let next = rig.after(ProgrammingOwner::Zoom, &closed, &fitted.result.continuity);
        assert_eq!(next.raw(), fitted.raw(), "{name}: new continuity is reused");
        rig.install(&profile);
    }
}
