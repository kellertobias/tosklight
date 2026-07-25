fn speed_command_events(scenario: &CommandHttpScenario, cursor: u64) -> usize {
	let light_application::EventReplay::Events(events) = scenario.state.application_events.replay(
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
		.audit_events
		.lock()
		.iter()
		.filter(|event| event.kind == "speed_group_command")
		.cloned()
		.collect()
}

#[tokio::test]
async fn v2_command_line_routes_decimal_relative_and_sync_through_speed_group_service() {
	let scenario = CommandHttpScenario::new().await;
	let cursor = scenario.state.application_events.latest_sequence();
	let attempts = scenario
		.state
		.speed_group_persistence_attempts
		.load(Ordering::Relaxed);

	let absolute = scenario
		.execute("speed-command-absolute", Some("SPD GRP 1 AT 128,5"))
		.await;
	assert_eq!(absolute.status(), StatusCode::OK);
	let absolute = json(absolute).await;
	assert_eq!(absolute["outcome"], "accepted");
	assert_eq!(absolute["command_line"]["text"], "FIXTURE");
	assert_eq!(scenario.state.speed_groups.lock()[0].manual_bpm(), 128.5);
	assert_eq!(speed_command_events(&scenario, cursor), 1);
	assert!(speed_compatibility_notifications(&scenario).is_empty());

	let relative = scenario
		.execute("speed-command-relative", Some("SPD GRP 1 AT - 8,25"))
		.await;
	assert_eq!(relative.status(), StatusCode::OK);
	assert_eq!(json(relative).await["outcome"], "accepted");
	assert_eq!(scenario.state.speed_groups.lock()[0].manual_bpm(), 120.25);

	let sync = scenario
		.execute("speed-command-sync", Some("SPD GRP 1 AT SPD GRP 2"))
		.await;
	assert_eq!(sync.status(), StatusCode::OK);
	assert_eq!(json(sync).await["outcome"], "accepted");
	let controllers = scenario.state.speed_groups.lock();
	assert_eq!(controllers[0].synchronized_with(), Some(2));
	assert_eq!(controllers[1].synchronized_with(), Some(1));
	assert_eq!(controllers[1].manual_bpm(), 120.25);
	drop(controllers);
	assert_eq!(speed_command_events(&scenario, cursor), 3);
	assert_eq!(
		scenario
			.state
			.speed_group_persistence_attempts
			.load(Ordering::Relaxed),
		attempts + 3
	);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn speed_command_replay_does_not_repeat_side_effects_or_erase_new_input() {
	let scenario = CommandHttpScenario::new().await;
	let cursor = scenario.state.application_events.latest_sequence();
	let history = history_len(&scenario);
	let first = scenario
		.execute("speed-command-once", Some("SPD GRP 1 AT 130"))
		.await;
	assert_eq!(first.status(), StatusCode::OK);
	assert_eq!(json(first).await["outcome"], "accepted");
	let attempts = scenario
		.state
		.speed_group_persistence_attempts
		.load(Ordering::Relaxed);
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
			.speed_group_persistence_attempts
			.load(Ordering::Relaxed),
		attempts
	);
	assert_eq!(history_len(&scenario), history + 1);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn retained_v1_programmer_execute_keeps_speed_group_payload_shape() {
	let scenario = CommandHttpScenario::new().await;
	let command = WsCommand {
		protocol_version: 1,
		request_id: "speed-v1".into(),
		session_id: scenario.session.id,
		expected_revision: None,
		command: "programmer.execute".into(),
		payload: serde_json::json!({"value":"SPD GRP 2 AT 99,5"}),
	};
	let cursor = scenario.state.application_events.latest_sequence();
	let response = dispatch_ws_command(&scenario.state, &scenario.session, command);
	assert!(response.ok, "{:?}", response.error);
	assert_eq!(scenario.state.speed_groups.lock()[1].manual_bpm(), 99.5);
	assert_eq!(speed_command_events(&scenario, cursor), 1);
	let notifications = speed_compatibility_notifications(&scenario);
	assert_eq!(notifications.len(), 1);
	assert_eq!(notifications[0].payload["command"], "SPD GRP 2 AT 99 . 5");
	assert_eq!(notifications[0].payload["groups"], serde_json::json!(["B"]));
	assert_eq!(notifications[0].payload["snapshots"].as_array().unwrap().len(), 1);
	let _ = std::fs::remove_dir_all(scenario.data_dir);
}
