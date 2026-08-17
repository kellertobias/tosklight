fn speed_command_events(scenario: &CommandHttpScenario, cursor: u64) -> usize {
	let light_application::EventReplay::Events(events) = scenario.state.events.replay(
		cursor,
		&light_application::EventFilter::default()
			.with_object(light_application::EventObject::speed_groups()),
	) else {
		return usize::MAX;
	};
	events.len()
}

fn speed_compatibility_notifications(scenario: &CommandHttpScenario) -> Vec<Event> {
	scenario
		.state
		.events.audit_events()
		.iter()
		.filter(|event| event.kind == "speed_group_command")
		.cloned()
		.collect()
}

async fn install_speed_group_binding_show(
	scenario: &CommandHttpScenario,
) -> (String, Uuid) {
	let show_id = scenario.create_and_open_show("Speed Group bindings").await;
	let cue_list_id = Uuid::new_v4();
	let cue_list = serde_json::json!({
		"id": cue_list_id,
		"name": "Rate source",
		"priority": 0,
		"mode": "chaser",
		"looped": true,
		"wrap_mode": "tracking",
		"restart_mode": "first_cue",
		"speed_multiplier": 1.5,
		"cues": [{
			"id": Uuid::new_v4(),
			"number": "1",
			"name": "Cue 1",
			"fade_millis": 0,
			"delay_millis": 0,
			"trigger": {"type":"manual"},
			"changes": [],
			"group_changes": []
		}],
		"future_cuelist_field": "preserved"
	});
	assert_eq!(
		scenario
			.put_active_object(
				&show_id,
				"cue_list",
				&cue_list_id.to_string(),
				0,
				cue_list,
			)
			.await
			.status(),
		StatusCode::OK
	);
	let playback = serde_json::json!({
		"number": 4,
		"name": "Rate source",
		"target": {"type":"cue_list","cue_list_id":cue_list_id},
		"buttons": ["go_minus","go","flash"],
		"button_count": 3,
		"fader": "master",
		"has_fader": true,
		"go_activates": true,
		"auto_off": false,
		"xfade_millis": 0,
		"color": "#20c997",
		"flash_release": "release_all",
		"protect_from_swap": false
	});
	assert_eq!(
		scenario
			.put_active_object(&show_id, "playback", "4", 0, playback.clone())
			.await
			.status(),
		StatusCode::OK
	);
	let mut virtual_playback = playback;
	virtual_playback["number"] = 1001.into();
	for page in [1, 2] {
		let virtual_playbacks = if page == 1 {
			serde_json::json!({"1001": virtual_playback.clone()})
		} else {
			serde_json::json!({})
		};
		assert_eq!(
			scenario
				.put_active_object(
					&show_id,
					"playback_page",
					&page.to_string(),
					0,
					serde_json::json!({
						"number": page,
						"name": format!("Page {page}"),
						"slots": {"1":4},
						"virtual_playbacks": virtual_playbacks
					}),
				)
				.await
				.status(),
			StatusCode::OK
		);
	}
	(show_id, cue_list_id)
}

fn stored_object_body(scenario: &CommandHttpScenario, kind: &str, id: &str) -> serde_json::Value {
	let (_, store) = active_show_store(&scenario.state).unwrap();
	store
		.objects(kind)
		.unwrap()
		.into_iter()
		.find(|object| object.id == id)
		.unwrap()
		.body
}

#[tokio::test]
async fn v2_command_line_routes_decimal_relative_and_sync_through_speed_group_service() {
	let scenario = CommandHttpScenario::new().await;
	let cursor = scenario.state.events.latest_sequence();
	let attempts = scenario
		.state
		.output
		.speed_group_persistence_attempts();

	let absolute = scenario
		.execute("speed-command-absolute", Some("SPD GRP 1 AT 128,5"))
		.await;
	assert_eq!(absolute.status(), StatusCode::OK);
	let absolute = json(absolute).await;
	assert_eq!(absolute["outcome"], "accepted");
	assert_eq!(absolute["command_line"]["text"], "FIXTURE");
	assert_eq!(scenario.state.output.speed_group_manual_bpm(0), 128.5);
	assert_eq!(speed_command_events(&scenario, cursor), 1);
	assert!(speed_compatibility_notifications(&scenario).is_empty());

	let relative = scenario
		.execute("speed-command-relative", Some("SPD GRP 1 AT - 8,25"))
		.await;
	assert_eq!(relative.status(), StatusCode::OK);
	assert_eq!(json(relative).await["outcome"], "accepted");
	assert_eq!(scenario.state.output.speed_group_manual_bpm(0), 120.25);

	let sync = scenario
		.execute("speed-command-sync", Some("SPD GRP 1 AT SPD GRP 2"))
		.await;
	assert_eq!(sync.status(), StatusCode::OK);
	assert_eq!(json(sync).await["outcome"], "accepted");
	assert_eq!(scenario.state.output.speed_group_controller(0).synchronized_with(), Some(2));
	assert_eq!(scenario.state.output.speed_group_controller(1).synchronized_with(), Some(1));
	assert_eq!(scenario.state.output.speed_group_manual_bpm(1), 120.25);
	assert_eq!(speed_command_events(&scenario, cursor), 3);
	assert_eq!(
		scenario
			.state
			.output
			.speed_group_persistence_attempts(),
		attempts + 3
	);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn speed_command_replay_does_not_repeat_side_effects_or_erase_new_input() {
	let scenario = CommandHttpScenario::new().await;
	let cursor = scenario.state.events.latest_sequence();
	let history = history_len(&scenario);
	let first = scenario
		.execute("speed-command-once", Some("SPD GRP 1 AT 130"))
		.await;
	assert_eq!(first.status(), StatusCode::OK);
	assert_eq!(json(first).await["outcome"], "accepted");
	let attempts = scenario
		.state
		.output
		.speed_group_persistence_attempts();
	let revision = json(scenario.get().await).await["revision"].as_u64().unwrap();
	assert_eq!(scenario.put("GROUP 9", revision).await.status(), StatusCode::OK);

	let replay = scenario
		.execute("speed-command-once", Some("SPD GRP 1 AT 130"))
		.await;
	assert_eq!(replay.status(), StatusCode::OK);
	assert_eq!(json(replay).await["outcome"], "accepted");
	assert_eq!(json(scenario.get().await).await["text"], "GROUP 9");
	assert_eq!(speed_command_events(&scenario, cursor), 1);
	assert_eq!(
		scenario
			.state
			.output
			.speed_group_persistence_attempts(),
		attempts
	);
	assert_eq!(history_len(&scenario), history + 1);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn typed_programmer_execute_keeps_speed_group_audit_payload_shape() {
	let scenario = CommandHttpScenario::new().await;
	let command = live_action_frame(
		&scenario.session,
		"speed-live-action",
		light_wire::v2::live_action::LiveAction::CommandLineExecute(
			light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
				value: "SPD GRP 2 AT 99,5".into(),
			},
		),
	);
	let cursor = scenario.state.events.latest_sequence();
	let response = dispatch_live_action(&scenario.state, &scenario.session, command);
	assert!(response.ok, "{:?}", response.error);
	assert_eq!(scenario.state.output.speed_group_manual_bpm(1), 99.5);
	assert_eq!(speed_command_events(&scenario, cursor), 1);
	let notifications = speed_compatibility_notifications(&scenario);
	assert_eq!(notifications.len(), 1);
	assert_eq!(notifications[0].payload["command"], "SPD GRP 2 AT 99 . 5");
	assert_eq!(notifications[0].payload["groups"], serde_json::json!(["B"]));
	assert_eq!(notifications[0].payload["snapshots"].as_array().unwrap().len(), 1);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn documented_cuelist_and_playback_addresses_replace_the_rate_source_without_selection() {
	let scenario = CommandHttpScenario::new().await;
	let (_show_id, cue_list_id) = install_speed_group_binding_show(&scenario).await;
	let active_show_id = scenario.state.active_show.current().as_ref().unwrap().id;
	select_playback(&scenario, scenario.session.desk.id, Some(77));

	for (request, command, expected) in [
		("bind-cuelist", "CUELIST 4 AT SPD GRP 2", "B"),
		("bind-current", "PBK 1 AT SPD GRP 3", "C"),
		("bind-explicit", "PBK 2 . 1 AT SPD GRP 4", "D"),
		("bind-virtual", "VPBK 1001 AT SPD GRP 5", "E"),
	] {
		let response = scenario.execute(request, Some(command)).await;
		assert_eq!(response.status(), StatusCode::OK, "{command}");
		let outcome = json(response).await;
		assert_eq!(outcome["outcome"], "accepted", "{command}: {outcome}");
		let body = stored_object_body(&scenario, "cue_list", &cue_list_id.to_string());
		assert_eq!(body["speed_group"], expected);
		assert_eq!(body["speed_multiplier"], 1.5);
		assert_eq!(body["future_cuelist_field"], "preserved");
		assert_eq!(
			scenario
				.state
				.output
				.snapshot()
				.cue_lists
				.iter()
				.find(|cue_list| cue_list.id.0 == cue_list_id)
				.and_then(|cue_list| cue_list.speed_group.as_deref()),
			Some(expected)
		);
		assert_eq!(
			scenario
				.state
				.installation
				.selected_playback(scenario.session.desk.id, active_show_id)
				.unwrap(),
			Some(77)
		);
	}
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn dynamic_binding_preserves_its_local_rate_and_rejects_invalid_sources_atomically() {
	let scenario = CommandHttpScenario::new().await;
	let (show_id, cue_list_id) = install_speed_group_binding_show(&scenario).await;
	let dynamic_id = Uuid::new_v4();
	let mut dynamic = serde_json::to_value(command_test_dynamic(dynamic_id, 29)).unwrap();
	dynamic["overall_speed_multiplier"] =
		serde_json::json!({"numerator": 3, "denominator": 2});
	dynamic["speed"] = serde_json::json!({
		"type":"speed_group",
		"group":"B",
		"beats_per_cycle":{"numerator":7,"denominator":2}
	});
	dynamic["future_definition_field"] = "preserved".into();
	assert_eq!(
		scenario
			.put_active_object(
				&show_id,
				"dynamic",
				&dynamic_id.to_string(),
				0,
				dynamic,
			)
			.await
			.status(),
		StatusCode::OK
	);

	let response = scenario
		.execute("bind-dynamic", Some("DYNAMIC 29 AT SPD GRP 3"))
		.await;
	assert_eq!(response.status(), StatusCode::OK);
	let body = stored_object_body(&scenario, "dynamic", &dynamic_id.to_string());
	assert_eq!(body["speed"]["group"], "C");
	assert_eq!(body["speed"]["beats_per_cycle"], serde_json::json!({"numerator":7,"denominator":2}));
	assert_eq!(body["overall_speed_multiplier"], serde_json::json!({"numerator":3,"denominator":2}));
	assert_eq!(body["future_definition_field"], "preserved");
	let runtime_dynamic = scenario
		.state
		.output
		.snapshot()
		.dynamics
		.iter()
		.find(|dynamic| dynamic.id == dynamic_id)
		.cloned()
		.unwrap();
	assert!(matches!(
		runtime_dynamic.speed,
		light_dynamics::DynamicSpeed::SpeedGroup {
			group: light_dynamics::SpeedGroup::C,
			beats_per_cycle: light_dynamics::Rational { numerator: 7, denominator: 2 }
		}
	));
	assert_eq!(
		runtime_dynamic.overall_speed_multiplier,
		light_dynamics::Rational { numerator: 3, denominator: 2 }
	);

	let before = stored_object_body(&scenario, "cue_list", &cue_list_id.to_string());
	for (request, command) in [
		("bind-empty", "PBK 9 AT SPD GRP 1"),
		("bind-range", "CUELIST 4 AT SPD GRP 6"),
		("bind-missing", "DYNAMIC 999 AT SPD GRP 1"),
	] {
		let response = scenario.execute(request, Some(command)).await;
		assert_eq!(response.status(), StatusCode::OK, "{command}");
		assert_eq!(json(response).await["outcome"], "rejected", "{command}");
	}
	assert_eq!(
		stored_object_body(&scenario, "cue_list", &cue_list_id.to_string()),
		before
	);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn live_command_surface_uses_the_same_playback_speed_group_binding() {
	let scenario = CommandHttpScenario::new().await;
	let (_show_id, cue_list_id) = install_speed_group_binding_show(&scenario).await;
	let frame = live_action_frame(
		&scenario.session,
		"bind-live-playback",
		light_wire::v2::live_action::LiveAction::CommandLineExecute(
			light_wire::v2::live_action::CommandLineExecuteLiveActionRequest {
				value: "PBK 1 AT SPD GRP 2".into(),
			},
		),
	);
	let response = dispatch_live_action(&scenario.state, &scenario.session, frame);
	assert!(response.ok, "{:?}", response.error);
	assert_eq!(
		stored_object_body(&scenario, "cue_list", &cue_list_id.to_string())["speed_group"],
		"B"
	);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}
