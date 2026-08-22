impl TemplateGroupScenario {
    fn seed_show_store(&self) {
        let fixtures = self
            .dimmers
            .iter()
            .chain(&self.profiles)
            .chain(&self.leds)
            .collect::<Vec<_>>();
        assert_eq!(fixtures.len(), 26);
        for fixture in fixtures {
            self.store
                .put_object(
                    "patched_fixture",
                    &fixture.fixture_id.0.to_string(),
                    &serde_json::to_value(fixture).unwrap(),
                    0,
                )
                .unwrap();
        }
        for group in &self.empty_groups {
            self.store
                .put_object(
                    "group",
                    &group.id,
                    &serde_json::to_value(group).unwrap(),
                    0,
                )
                .unwrap();
        }
        let preset = template_group_preset();
        self.store
            .put_object(
                "preset",
                "0.1",
                &serde_json::to_value(&preset).unwrap(),
                0,
            )
            .unwrap();
        let cue_list = template_cue_list(self.cue_list_id, &preset);
        self.store
            .put_object(
                "cue_list",
                &self.cue_object_id(),
                &serde_json::to_value(&cue_list).unwrap(),
                0,
            )
            .unwrap();
    }

    fn activate_and_verify_empty_groups(&self) {
        self.state.active_show.replace_current(Some(self.entry.clone()));
        self.state
            .output.replace_snapshot(load_engine_snapshot(&self.entry).unwrap())
            .unwrap();
        self.state
            .output.execute_playback(EnginePlaybackCommand::CueList {
                id: self.cue_list_id,
                action: light_engine::CueListPlaybackAction::Go,
            })
            .unwrap();
        let dark = self.render();
        for fixture in &self.dimmers {
            assert_eq!(dark[usize::from(fixture.address.unwrap() - 1)], 0);
        }
        for fixture in &self.profiles {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(dark[offset + 4], 0);
            assert_eq!(&dark[offset + 6..=offset + 8], &[0; 3]);
        }
        for fixture in &self.leds {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&dark[offset..offset + 4], &[0; 4]);
        }
    }

    fn populate_groups_and_verify_white(&self) -> light_output::DmxFrame {
        for group in &self.populated_groups {
            self.store
                .put_object(
                    "group",
                    &group.id,
                    &serde_json::to_value(group).unwrap(),
                    1,
                )
                .unwrap();
        }
        self.reload_snapshot();
        let frame = self.render();
        for fixture in &self.dimmers {
            assert_eq!(frame[usize::from(fixture.address.unwrap() - 1)], 255);
        }
        for fixture in &self.profiles {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&frame[offset + 4..=offset + 8], &[255; 5]);
        }
        for fixture in &self.leds {
            let offset = usize::from(fixture.address.unwrap() - 1);
            assert_eq!(&frame[offset..offset + 4], &[255; 4]);
        }
        frame
    }

    fn exercise_first_preload(&self, white_frame: &light_output::DmxFrame) {
        self.set_preload(0.25, 0.75);
        assert_eq!(&self.render(), white_frame);
        self.state.programming.activate_preload(self.session_id);
        let frame = self.render();
        let address = self.profiles[0].address.unwrap();
        assert_eq!(template_word(&frame, address), 16_384);
        assert_eq!(template_word(&frame, address + 2), 49_151);
    }

    async fn store_active_preload(&self) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v2/preload/record")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", self.token),
                )
                .header("x-tosk-show", self.entry.id.0.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": Uuid::new_v4().to_string(),
                        "action": {
                            "type": "cue",
                            "cue_list_id": self.cue_object_id(),
                            "expected_revision": 1,
                            "cue_number": "2",
                            "name": "Preloaded position"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json(response).await["object"]["revision"], 2);
        let persisted = self.stored_cue_list();
        assert_eq!(persisted.cues.len(), 2);
        assert!(persisted.cues[1].group_changes.iter().any(|change| {
            change.group_id == "profile"
                && *change.attribute.0 == *"pan"
                && change
                    .value
                    .as_ref()
                    .and_then(light_core::AttributeValue::normalized)
                    == Some(0.25)
        }));
        let programmer = self.state.programming.get(self.session_id).unwrap();
        assert!(programmer.preload_active.is_empty());
        assert!(programmer.preload_group_active.is_empty());
    }

    fn exercise_second_preload_and_recall(&self) {
        self.set_preload(0.8, 0.2);
        self.state
            .output.execute_playback(EnginePlaybackCommand::CueList {
                id: self.cue_list_id,
                action: light_engine::CueListPlaybackAction::Jump(cue("2")),
            })
            .unwrap();
        let address = self.profiles[0].address.unwrap();
        let recalled = self.render();
        assert_eq!(template_word(&recalled, address), 16_384);
        assert_eq!(template_word(&recalled, address + 2), 49_151);
        self.state.programming.activate_preload(self.session_id);
        let second_go = self.render();
        assert_eq!(template_word(&second_go, address), 52_428);
        assert_eq!(template_word(&second_go, address + 2), 13_107);
    }

    fn verify_export(&self) {
        let export = self.data_dir.join("template-group-scenario.show");
        self.store.backup_to(&export).unwrap();
        validate_show_file(&export).unwrap();
        let reopened = ShowEntry {
            path: export.to_string_lossy().into_owned(),
            ..self.entry.clone()
        };
        let snapshot = load_engine_snapshot(&reopened).unwrap();
        assert_eq!(snapshot.fixtures.len(), 26);
        assert_eq!(snapshot.groups.len(), 3);
        assert_eq!(snapshot.cue_lists[0].cues.len(), 2);
    }

    fn verify_late_patch_rendering(&self) {
        let extra_profiles = vec![template_profile(7, 141), template_profile(8, 153)];
        for fixture in &extra_profiles {
            self.store
                .put_object(
                    "patched_fixture",
                    &fixture.fixture_id.0.to_string(),
                    &serde_json::to_value(fixture).unwrap(),
                    0,
                )
                .unwrap();
        }
        let expanded_profile = light_programmer::GroupDefinition {
            fixtures: self.populated_groups[2]
                .fixtures
                .iter()
                .copied()
                .chain(extra_profiles.iter().map(|fixture| fixture.fixture_id))
                .collect(),
            ..self.populated_groups[2].clone()
        };
        let expected_revision = self
            .store
            .objects("group")
            .unwrap()
            .into_iter()
            .find(|object| object.id == "profile")
            .expect("stored profile group")
            .revision;
        self.store
            .put_object(
                "group",
                "profile",
                &serde_json::to_value(expanded_profile).unwrap(),
                expected_revision,
            )
            .unwrap();
        self.state.programming.release_preload(self.session_id);
        self.reload_snapshot();
        self.state
            .output.execute_playback(EnginePlaybackCommand::CueList {
                id: self.cue_list_id,
                action: light_engine::CueListPlaybackAction::Jump(cue("2")),
            })
            .unwrap();
        let expanded = self.render();
        for fixture in &extra_profiles {
            let address = fixture.address.unwrap();
            let offset = usize::from(address - 1);
            assert_eq!(expanded[offset + 4], 255);
            assert_eq!(&expanded[offset + 6..=offset + 8], &[255; 3]);
            assert_eq!(template_word(&expanded, address), 16_384);
            assert_eq!(template_word(&expanded, address + 2), 49_151);
        }
    }

    fn verify_stored_group_values(&self) {
        let cue_list = self.stored_cue_list();
        assert!(cue_list.cues[1].group_changes.iter().any(|change| {
            change.group_id == "profile" && *change.attribute.0 == *"pan"
        }));
        assert_eq!(
            cue_list.cues[1]
                .group_changes
                .iter()
                .filter(|change| change.group_id == "profile")
                .count(),
            2
        );
    }

    fn set_preload(&self, pan: f32, tilt: f32) {
        self.state
            .programming
            .set_modes(self.session_id, Some(true), None, None, None);
        for (attribute, value) in [("pan", pan), ("tilt", tilt)] {
            self.state.programming.set_preload_group(
                self.session_id,
                "profile".into(),
                light_core::AttributeKey(attribute.into()),
                light_core::AttributeValue::Normalized(value),
            );
        }
    }

    fn reload_snapshot(&self) {
        self.state
            .output.replace_snapshot(load_engine_snapshot(&self.entry).unwrap())
            .unwrap();
    }

    fn render(&self) -> light_output::DmxFrame {
        self.state
            .output.render(RenderOptions::default())
            .unwrap()
            .universes[&1]
    }

    fn stored_cue_list(&self) -> light_playback::CueList {
        serde_json::from_value(
            self.store
                .objects("cue_list")
                .unwrap()
                .into_iter()
                .next()
                .unwrap()
                .body,
        )
        .unwrap()
    }
}

#[tokio::test]
async fn template_groups_preload_store_reload_and_late_patch_render_end_to_end() {
    let scenario = TemplateGroupScenario::new().await;
    scenario.seed_show_store();
    scenario.activate_and_verify_empty_groups();
    let white_frame = scenario.populate_groups_and_verify_white();
    scenario.exercise_first_preload(&white_frame);
    scenario.store_active_preload().await;
    scenario.exercise_second_preload_and_recall();
    scenario.verify_export();
    scenario.verify_late_patch_rendering();
    scenario.verify_stored_group_values();
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
