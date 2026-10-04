//! TL-560 / plan "Mandatory regression" for Presets (the Cue half is
//! `tests_lifecycle::recorded_semantic_cues_survive_save_reload_fixture_replacement_and_new_live_group_members`)
//! and the acceptance UV scenario "record visible color plus UV, save/reload, replace RGBWAUV
//! with RGB/CMY and back, add fixtures to a live group".
//!
//! Presets are recorded from a live Group through the real Preset writer (one shared whole
//! colour is consolidated into a universal Color Preset by design). Every step reopens the
//! SQLite show, proves the stored Preset bodies unchanged, opens a fresh contract-1 desk, selects
//! the live Group and recalls the Preset through the real `ProgrammingService::
//! handle_preset_recall` planner. The recall environment port below resolves the Preset and the
//! Group graph from the reopened portable document (a test port; the headless port additionally
//! carries Stage positions and logical-head expansions). Output is the production Live family
//! frame and its encoded universe bytes.
use super::*;
use light_application::{
    ProgrammingPresetRecallEnvironment, ProgrammingPresetRecallPorts,
    ProgrammingPresetRecallRequest, ProgrammingPresetRecallRevisionExpectation,
};
use light_core::programming::UvIntent;
use light_programmer::{PresetAddress, PresetFamily, SelectionExpression, SelectionRule};
use std::collections::HashMap;

pub(super) struct Recall<'a> {
    pub(super) show: &'a Show,
}

impl ProgrammingPresetRecallPorts for Recall<'_> {
    fn authorize_preset_recall(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn preset_recall_environment(
        &self,
        _context: &ActionContext,
        request: &ProgrammingPresetRecallRequest,
    ) -> Result<ProgrammingPresetRecallEnvironment, ActionError> {
        let document = self.show.document();
        let object_id = request.address.storage_key();
        let object = document
            .object("preset", &object_id)
            .ok_or_else(|| ActionError::new(ActionErrorKind::NotFound, "missing Preset"))?;
        let raw_body = object.body().clone();
        let preset: light_programmer::Preset = serde_json::from_value(raw_body.clone()).unwrap();
        let snapshot = self.show.compile();
        let targets: Vec<FixtureId> = snapshot.fixtures.iter().map(|f| f.fixture_id).collect();
        Ok(ProgrammingPresetRecallEnvironment {
            supported_programming_contract: light_core::programming::PROGRAMMING_CONTRACT_VERSION,
            show_id: self.show.ports.show_id,
            show_revision: document.revision(),
            object_id,
            object_revision: object.revision(),
            address: request.address,
            raw_body: Arc::new(raw_body),
            preset: Arc::new(preset),
            resolved_aim: None,
            groups: Arc::new(
                snapshot
                    .groups
                    .iter()
                    .map(|group| (group.id.clone(), group.clone()))
                    .collect(),
            ),
            stage_positions: Arc::new(HashMap::new()),
            target_expansions: Arc::new(targets.iter().map(|id| (*id, vec![*id])).collect()),
            selectable_targets: Arc::new(targets),
            programmer_fade_millis: 0,
        })
    }

    fn persist_preset_recall(
        &self,
        _context: &ActionContext,
        _operation: &'static str,
    ) -> Option<String> {
        None
    }
}

/// A fresh desk on the reopened show: select the live Group and recall Color Preset `number`.
fn recall(show: &Show, desk: &Desk, session: SessionId, members: &[FixtureId], number: u32) {
    let programming = ProgrammingService::new(
        desk.programmers.clone(),
        EventBus::new(16),
        Arc::new(HighlightRegistry::default()),
    );
    desk.programmers.select_expression(
        session,
        members.to_vec(),
        SelectionExpression::LiveGroup {
            group_id: GROUP.into(),
            rule: SelectionRule::All,
        },
    );
    let current = ProgrammingPresetRecallRevisionExpectation::Current;
    programming
        .handle_preset_recall(
            ActionEnvelope {
                context: operator(session, "recall"),
                command: ProgrammingPresetRecallRequest {
                    show_id: show.ports.show_id,
                    address: PresetAddress::new(PresetFamily::Color, number).unwrap(),
                    expected_preset_revision: current,
                    expected_show_revision: current,
                    expected_values_revision: current,
                    expected_preload_values_revision: current,
                    expected_capture_mode_revision: current,
                    expected_selection_revision: current,
                },
            },
            &Recall { show },
        )
        .unwrap();
    desk.clock.advance_millis(600_000);
}

fn presets(show: &Show) -> Vec<(String, Value)> {
    let mut presets: Vec<_> = show
        .document()
        .objects_of_kind("preset")
        .map(|object| (object.key().id().to_owned(), object.body().clone()))
        .collect();
    presets.sort_by(|a, b| a.0.cmp(&b.0));
    presets
}

fn record_group_preset(show: &Show, value: &ColorIntent, number: u32) {
    show.programmers.set_group(
        show.session,
        GROUP.into(),
        ProgrammingOwner::Color.key(),
        program(value),
    );
    record_preset(
        show,
        &show.programming,
        show.session,
        PresetFamily::Color,
        number,
    );
    show.programmers.clear(show.session);
    show.programmers.start(show.session);
}

#[tokio::test]
async fn magenta_and_warm_white_presets_survive_reopen_rgbw_cmy_hybrid_replacement_and_group_growth()
 {
    let show = Show::new();
    let (a, b) = (FixtureId::new(), FixtureId::new());
    show.patch(&fixture(&rgb(), a, 1, 1));
    show.group(&[a]);
    record_group_preset(&show, &magenta(), 1);
    record_group_preset(&show, &warm_white(), 2);
    let recorded = presets(&show);
    assert_eq!(recorded.len(), 2);
    let stored = [(1, magenta()), (2, warm_white())];
    for ((_, body), (_, intent)) in recorded.iter().zip(&stored) {
        assert_eq!(
            body["universal_values"]["color"],
            serde_json::to_value(program(intent)).unwrap(),
            "the exact requested intent, no native recipe or achieved output"
        );
    }

    let replacements = [rgb(), rgbw_white_on(), cmy_wheel(), hybrid()];
    for (step, profile) in replacements.iter().enumerate() {
        let fixtures = if step == 0 {
            vec![fixture(profile, a, 1, 1)]
        } else {
            vec![fixture(profile, a, 1, 1), fixture(profile, b, 2, 40)]
        };
        for patched in &fixtures {
            show.patch(patched);
        }
        let members: Vec<_> = fixtures.iter().map(|f| f.fixture_id).collect();
        show.group(&members);
        assert_eq!(
            presets(&show),
            recorded,
            "{}: Presets unchanged",
            profile.name
        );
        for (number, intent) in &stored {
            let desk = Desk::open(show.compile());
            let session = SessionId::new();
            desk.programmers.start(session);
            recall(&show, &desk, session, &members, *number);
            let frame = desk.frame();
            for patched in &fixtures {
                let id = patched.fixture_id;
                assert_eq!(
                    desk.played(id, ProgrammingOwner::Color),
                    Some(program(intent)),
                    "{}: the recalled intent",
                    profile.name
                );
                let written: Vec<u32> = frame
                    .family(ProgrammingOwner::Color, id)
                    .into_iter()
                    .map(|(channel, _)| channel)
                    .collect();
                assert_eq!(
                    written,
                    color_channels(profile),
                    "{}: every destination Color control is decided",
                    profile.name
                );
                let native = frame.assert_encoded(patched);
                if step == 1 {
                    assert_ne!(native[4], 204, "White is fitted, never left at its seed");
                }
            }
        }
    }
}

fn magenta_with_uv() -> ColorIntent {
    ColorIntent {
        uv: UvIntent { amount: 0.45 },
        ..magenta()
    }
}

fn uv_channel(profile: &FixtureProfile) -> Option<usize> {
    profile.modes[0]
        .channels
        .iter()
        .position(|c| c.attribute.0.contains("uv"))
}

#[tokio::test]
async fn visible_color_plus_uv_survives_uv_loss_and_readdition_with_live_group_growth() {
    let show = Show::new();
    let (u, v) = (FixtureId::new(), FixtureId::new());
    show.patch(&fixture(&rgbwauv(None), u, 1, 1));
    show.group(&[u]);
    record_group_preset(&show, &magenta_with_uv(), 1);
    record_group_preset(&show, &magenta(), 2);
    let recorded = presets(&show);

    let replacements = [rgbwauv(None), rgb(), cmy_wheel(), rgbwauv(None)];
    for (step, profile) in replacements.iter().enumerate() {
        let fixtures = if step == 0 {
            vec![fixture(profile, u, 1, 1)]
        } else {
            // A member added to the live Group after recording, replaced alongside.
            vec![fixture(profile, u, 1, 1), fixture(profile, v, 2, 40)]
        };
        for patched in &fixtures {
            show.patch(patched);
        }
        let members: Vec<_> = fixtures.iter().map(|f| f.fixture_id).collect();
        show.group(&members);
        assert_eq!(
            presets(&show),
            recorded,
            "{}: Presets unchanged",
            profile.name
        );
        let desk = Desk::open(show.compile());
        let session = SessionId::new();
        desk.programmers.start(session);
        recall(&show, &desk, session, &members, 1);
        let with_uv = desk.frame();
        for &id in &members {
            assert_eq!(
                desk.played(id, ProgrammingOwner::Color),
                Some(program(&magenta_with_uv())),
                "{}: the stored UV amount never changes",
                profile.name
            );
        }
        recall(&show, &desk, session, &members, 2);
        let without_uv = desk.frame();
        for patched in &fixtures {
            let uv = with_uv.assert_encoded(patched);
            let visible_only = without_uv.assert_encoded(patched);
            match uv_channel(profile) {
                Some(channel) => {
                    assert_eq!(
                        uv[channel],
                        (0.45f64 * 255.).round() as u32,
                        "{}: UV starts at the stored amount",
                        profile.name
                    );
                    assert_eq!(visible_only[channel], 0);
                    let visible = |native: &[u32]| {
                        let mut native = native.to_vec();
                        native.remove(channel);
                        native
                    };
                    assert_eq!(
                        visible(&uv),
                        visible(&visible_only),
                        "{}: UV is independent of the visible fit",
                        profile.name
                    );
                }
                None => assert_eq!(
                    uv, visible_only,
                    "{}: no UV emitter, no violet substitute",
                    profile.name
                ),
            }
        }
    }
}
