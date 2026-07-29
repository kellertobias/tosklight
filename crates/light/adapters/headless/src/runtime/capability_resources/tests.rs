use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn standalone_session(token: &str, desk_id: Uuid) -> Session {
        Session {
            id: SessionId::new(),
            user: DeskUser {
                id: light_core::UserId(Uuid::new_v4()),
                name: "Standalone operator".into(),
                enabled: true,
            },
            token: token.into(),
            connected: true,
            desk: ControlDesk {
                id: desk_id,
                name: "Standalone desk".into(),
                osc_alias: "standalone".into(),
                columns: 8,
                rows: 1,
                buttons: 3,
                playback_layout: None,
            },
        }
    }

    #[test]
    fn session_resource_operates_without_the_server_state_bag() {
        let sessions = SessionResource::new();
        let desk_id = Uuid::new_v4();
        let session = standalone_session("standalone-token", desk_id);
        let client_id = Uuid::new_v4();

        sessions.insert_session(session.clone());
        sessions.bind_client(session.id, client_id);
        assert_eq!(
            sessions.session_for_token("standalone-token").unwrap().id,
            session.id
        );
        assert!(sessions.session_token_matches(session.id, "standalone-token"));
        assert!(sessions.client_connected(client_id));
        assert!(sessions.desk_in_use(desk_id));

        let context = file_manager::FileInputContext {
            instance_id: "standalone-files".into(),
            action: file_manager::FileInputAction::Copy,
            session_id: session.id,
            desk_id,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(60),
        };
        sessions
            .try_claim_file_input_context(context, || Ok(()))
            .unwrap();
        assert!(sessions.file_input_context(desk_id).is_some());
        assert!(matches!(
            sessions.route_file_input(
                desk_id,
                "enter",
                std::time::Instant::now() + std::time::Duration::from_secs(60),
            ),
            SessionFileInputRoute::Dispatch(_)
        ));
        assert!(sessions.release_session_file_input(&session).is_some());

        assert_eq!(sessions.unbind_client(session.id), Some(client_id));
        assert_eq!(
            sessions.remove_session(session.id).unwrap().token,
            "standalone-token"
        );
        assert_eq!(sessions.session_count(), 0);
    }

    #[test]
    fn event_resource_operates_without_the_server_state_bag() {
        let events = EventResource::new(EventBus::new(4));
        assert_eq!(events.latest_sequence(), 0);
        assert_eq!(
            events.record_audit("standalone", serde_json::json!({"value": 1})),
            1
        );
        assert_eq!(events.audit_after(0)[0].kind, "standalone");

        let filter = light_application::EventFilter::for_desk(Uuid::new_v4());
        let subscription = events.subscribe(
            filter.clone(),
            light_application::SubscriptionOptions::default(),
        );
        events.publish(light_application::EventDraft::configuration_changed(
            light_application::NotificationRevision { revision: 1 },
        ));
        assert_eq!(events.latest_sequence(), 1);
        let light_application::EventReplay::Events(replayed) = events.replay(0, &filter) else {
            panic!("standalone event resource should retain its event");
        };
        assert_eq!(replayed.len(), 1);
        assert!(subscription.try_next().is_some());
    }

    #[test]
    fn installation_resource_operates_without_the_server_state_bag() {
        let mut installation = InstallationResource::new(
            DeskStore::open(":memory:").unwrap(),
            light_fixture::FixtureLibrary::open(":memory:").unwrap(),
            PathBuf::new(),
            DeskConfiguration::default(),
            None,
        );

        assert!(installation.fixture_definitions().unwrap().is_empty());
        assert!(installation.fixture_profiles().unwrap().is_empty());
        assert!(installation.fixture_library_warnings().unwrap().is_empty());
        assert_eq!(installation.data_dir(), std::path::Path::new(""));
        assert_eq!(
            installation.configuration().frame_rate_hz,
            DeskConfiguration::default().frame_rate_hz
        );
        assert!(installation.desk_token_matches(None));
        installation.set_desk_token("standalone-token");
        assert!(installation.desk_token_matches(Some("standalone-token")));
        assert!(!installation.desk_token_matches(Some("wrong-token")));

        installation.set_setting("standalone", "available").unwrap();
        assert_eq!(
            installation.setting("standalone").unwrap().as_deref(),
            Some("available")
        );
    }

    #[test]
    fn media_resource_operates_without_the_server_state_bag() {
        let media = MediaResource::default();
        let fixture_id = light_core::FixtureId::new();
        let key = ThumbnailKey {
            fixture: fixture_id.0.to_string(),
            library_type: 1,
            library: LibraryId::ROOT,
            element: 7,
        };
        media
            .put_thumbnails([(
                key.clone(),
                light_media::MediaImage {
                    format: light_media::ImageFormat::Jpeg,
                    width: 1,
                    height: 1,
                    bytes: vec![1],
                },
            )])
            .unwrap();
        media.record_status(fixture_id, None);

        assert_eq!(media.thumbnail(&key).unwrap().image.bytes, vec![1]);
        assert!(media.status(fixture_id).online);

        media.invalidate_fixture(fixture_id);
        assert!(media.thumbnail(&key).is_none());
        assert!(!media.status(fixture_id).online);
    }

    #[test]
    fn highlight_resource_operates_without_the_server_state_bag() {
        let highlight = HighlightResource::new(Arc::new(HighlightRegistry::default()));
        let session_id = SessionId::new();
        let fixture_id = light_core::FixtureId::new();

        assert!(highlight.set_patch_preview(session_id, HashSet::from([fixture_id])));
        assert_eq!(highlight.output_fixtures(), HashSet::from([fixture_id]));

        highlight.remove_patch_preview(session_id);
        assert!(highlight.output_fixtures().is_empty());
    }

    #[test]
    fn integration_resource_operates_without_the_server_state_bag() {
        let integrations =
            IntegrationResource::new(Arc::new(matter::MatterBridgeAdapter::default()), None, None);
        let source: SocketAddr = "127.0.0.1:19010".parse().unwrap();
        let session_id = SessionId::new();
        integrations.register_osc_subscriber(
            "standalone".into(),
            OscSubscriber {
                desk_alias: "main".into(),
                target: source,
                command_source: source,
                session_id,
                last_seen: Instant::now(),
                shifted: false,
                shift_held: false,
                update_record_started: None,
                update_first_release: None,
                last_highlight_action: None,
            },
        );

        assert!(integrations.hardware_connected());
        assert_eq!(
            integrations
                .osc_subscriber_for_source(source)
                .unwrap()
                .session_id,
            session_id
        );
        integrations.set_shift(source, true);
        assert!(integrations.osc_subscriber("standalone").unwrap().shifted);
        assert_eq!(
            integrations
                .unregister_osc_subscriber("standalone")
                .unwrap()
                .session_id,
            session_id
        );
        assert!(!integrations.hardware_connected());
    }

    #[test]
    fn output_resource_operates_without_the_server_state_bag() {
        let events = EventBus::default();
        let output = OutputResource::new(
            OutputRuntimeService::new(events.clone()),
            SpeedGroupService::new(events),
            Arc::new(Engine::new(ProgrammerRegistry::default())),
            Arc::new(std::sync::Mutex::new(OutputHealth::default())),
            Arc::new(AtomicU16::new(44)),
            OutputControlCapability::new(Arc::new(Mutex::new(OutputControl::default()))),
            Arc::new(Mutex::new(TimecodeRouter::default())),
            None,
            Arc::default(),
            None,
            Arc::new(Mutex::new(std::array::from_fn(|index| {
                SpeedGroupController::new(
                    default_speed_groups()[index],
                    SoundToLightConfig::default(),
                )
                .unwrap()
            }))),
            Arc::new(Mutex::new(light_dynamics::DynamicRuntime::default())),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(super::visualization_frame::VisualizationFrameHub::default()),
        );

        output.apply_runtime_control(Some(0.5), Some(true)).unwrap();
        assert_eq!(output.control_projection().revision, 1);
        assert_eq!(output.control_projection().grand_master, 0.5);
        assert!(output.control_projection().blackout);

        output.set_dmx_override(1, 7, Some(201));
        assert_eq!(output.dmx_override(1, 7), Some(201));
        output.set_manual_speed_group(0, 128.5, 0, true).unwrap();
        assert_eq!(output.speed_group_manual_bpm(0), 128.5);
    }

    #[tokio::test]
    async fn lifecycle_resource_operates_without_the_server_state_bag() {
        let lifecycle = LifecycleResource::new(CancellationToken::new());
        let cancellation = lifecycle.cancellation_token();
        assert!(!lifecycle.is_shutdown_requested());

        lifecycle.request_shutdown();

        cancellation.cancelled().await;
        assert!(lifecycle.is_shutdown_requested());
    }

    #[test]
    fn lifecycle_resource_rejects_work_when_supervisor_queue_is_full() {
        let lifecycle = LifecycleResource::new(CancellationToken::new());
        for _ in 0..RUNTIME_TASK_QUEUE_CAPACITY {
            lifecycle.schedule(async { Ok(()) }).unwrap();
        }

        let error = lifecycle.schedule(async { Ok(()) }).unwrap_err();

        assert_eq!(error.to_string(), "runtime task supervisor queue is full");
    }

    #[tokio::test]
    async fn replay_resource_preserves_fingerprint_and_replayed_metadata() {
        let resource = ReplayResource::default();
        let key = desk_management_v2::ReplayKey::new(
            SessionId(Uuid::from_u128(1)),
            "test-edit",
            "request-1",
        );
        let fingerprint = serde_json::json!({"value": 1});
        resource
            .insert_desk_management(
                key.clone(),
                fingerprint.clone(),
                serde_json::json!({"request_id":"request-1","replayed":false}),
            )
            .await;

        let replayed = resource
            .lookup_desk_management(&key, &fingerprint)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(replayed["replayed"], true);

        let conflict = resource
            .lookup_desk_management(&key, &serde_json::json!({"value": 2}))
            .await
            .unwrap_err();
        assert_eq!(conflict.status, StatusCode::CONFLICT);
        assert_eq!(
            conflict.message,
            "request_id was already used for a different edit"
        );
    }

    #[tokio::test]
    async fn independent_replay_capabilities_do_not_share_a_lock() {
        let resource = ReplayResource::default();
        let _show_library_guard = resource.show_library.lock().await;
        let key = desk_management_v2::ReplayKey::new(
            SessionId(Uuid::from_u128(2)),
            "test-edit",
            "request-2",
        );

        tokio::time::timeout(
            Duration::from_millis(100),
            resource.lookup_desk_management(&key, &serde_json::json!({"value": 1})),
        )
        .await
        .expect("an unrelated replay capability must not wait for the show-library cache")
        .unwrap();
    }

    #[test]
    fn programming_resource_operates_without_app_state_or_registry_access() {
        let programmers = ProgrammerRegistry::default();
        let service = ProgrammingService::new(
            programmers.clone(),
            EventBus::default(),
            Arc::new(HighlightRegistry::default()),
        );
        let resource = ProgrammingResource::new(programmers, service);
        let session_id = SessionId(Uuid::from_u128(31));
        let user_id = light_core::UserId(Uuid::from_u128(32));
        let fixture_id = light_core::FixtureId(Uuid::from_u128(33));

        resource.start(session_id, user_id);
        resource.select(session_id, [fixture_id]);
        assert_eq!(
            resource
                .selection(session_id)
                .expect("standalone selection")
                .selected,
            vec![fixture_id]
        );

        resource
            .with_staged_command(session_id, |staged| {
                staged.set_command_line(session_id, "FIXTURE 1".into());
                Ok::<_, String>(())
            })
            .unwrap();
        assert_eq!(
            resource
                .get(session_id)
                .expect("standalone programmer")
                .command_line,
            "FIXTURE 1"
        );
    }

    #[tokio::test]
    async fn active_show_resource_operates_without_the_server_state_bag() {
        let events = EventBus::default();
        let service = ActiveShowService::new(events);
        let resource = ActiveShowResource::new(
            ActiveShowCoordinator::new(),
            Arc::default(),
            Some("recovery".into()),
            service.clone(),
            ShowPatchService::new(service.clone()),
            SelectiveShowImportService::new(service),
        );
        let show = ShowEntry {
            id: light_core::ShowId::new(),
            name: "Standalone".into(),
            path: "standalone.show".into(),
            revision: 3,
            updated_at: String::new(),
            revision_copy: None,
        };

        resource.replace_current(Some(show.clone()));
        let current = resource.current().expect("standalone show");
        assert_eq!(current.id, show.id);
        assert_eq!(current.name, show.name);
        assert_eq!(resource.error().as_deref(), Some("recovery"));
        resource.set_error(None);
        assert!(resource.error().is_none());

        let permit = resource.acquire().await;
        assert!(resource.try_acquire().is_err());
        drop(permit);
        assert!(resource.try_acquire().is_ok());
    }
}
