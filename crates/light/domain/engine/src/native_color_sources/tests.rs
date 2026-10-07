use super::*;
use light_core::{AttributeKey, Xyz, programming::NativeColorRecipe};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelFunction, ChannelResolution, ColorPhysicalModel,
    FixtureChannel, FixtureSplit, HeadOpticalPath, NativeColorBinding, NativeColorValue,
    OpticalEmitter, OpticalEmitterBand, OpticalProvenance, OpticalSource,
};

fn profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Retained original Color source".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let attribute = AttributeKey("color.uv".into());
    let function = ChannelFunction::continuous("UV", attribute.clone(), 255);
    let channel = FixtureChannel {
        id: Uuid::new_v4(),
        head_id: mode.heads[0].id,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute,
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![function],
    };
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id: mode.heads[0].id,
            controls: vec![channel.id],
            source: OpticalSource::Additive {
                emitters: vec![OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: "UV".into(),
                    binding: NativeColorBinding {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                    },
                    xyz: Some(Xyz {
                        x: 0.1,
                        y: 0.2,
                        z: 0.3,
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Ultraviolet,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: OpticalProvenance::default(),
                }],
            },
            filters: vec![],
            measurements: vec![],
        }],
    });
    mode.channels.push(channel);
    profile.validate().unwrap();
    profile
}

fn identity(profile: &FixtureProfile) -> NativeColorIdentity {
    profile
        .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
        .unwrap()
}

fn key(profile: &FixtureProfile, digest: &str) -> NativeColorSourceRevisionKey {
    NativeColorSourceRevisionKey {
        profile_id: profile.id,
        revision: u64::from(profile.revision),
        raw_store_digest: digest.into(),
    }
}

#[test]
fn expected_missing_and_unverified_originals_are_typed_capability_gaps() {
    use light_dynamics::{NativeColorModelCapability, NativeColorUnavailableReason};
    let profile = profile();
    let source = identity(&profile);
    let unavailable =
        |catalogue: &NativeColorSourceCatalog, source: &NativeColorIdentity| match catalogue
            .resolve_capability(source)
            .unwrap()
        {
            NativeColorModelCapability::Unavailable(reason) => reason.reason,
            NativeColorModelCapability::Available(_) => panic!("expected unavailable original"),
        };
    assert_eq!(
        unavailable(&NativeColorSourceCatalog::default(), &source),
        NativeColorUnavailableReason::CatalogueNotPrepared
    );
    let empty = NativeColorSourceCatalog::from_revisions([]).unwrap();
    assert_eq!(
        unavailable(&empty, &source),
        NativeColorUnavailableReason::MissingRevision
    );
    let entry = NativeColorSourceCatalog::compile_revision(key(&profile, "digest"), None, || {
        Ok(profile.clone())
    });
    let catalogue = NativeColorSourceCatalog::from_revisions([entry]).unwrap();
    assert!(matches!(
        catalogue.resolve_capability(&source).unwrap(),
        NativeColorModelCapability::Available(_)
    ));
    let mut changed = source.clone();
    changed.profile_digest = "not-the-recorded-original".into();
    assert_eq!(
        unavailable(&catalogue, &changed),
        NativeColorUnavailableReason::UnverifiedOriginal
    );
    changed = source.clone();
    changed.path_id = Uuid::new_v4();
    assert_eq!(
        unavailable(&catalogue, &changed),
        NativeColorUnavailableReason::MissingPath
    );
    changed.profile_id = Uuid::nil();
    assert!(
        empty.resolve_capability(&changed).is_err(),
        "invalid identity is not a capability warning"
    );
}

fn entry(profile: &FixtureProfile) -> Arc<NativeColorSourceRevision> {
    NativeColorSourceCatalog::compile_revision(key(profile, "raw store digest"), None, || {
        Ok(profile.clone())
    })
}

fn recipe(profile: &FixtureProfile) -> NativeColorRecipe {
    let channel = &profile.modes[0].channels[0];
    NativeColorRecipe {
        source: identity(profile),
        channels: vec![NativeColorValue {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            raw: 255,
        }],
        spreads: vec![],
    }
}

#[test]
fn exact_original_models_survive_replacement_without_any_patched_fixture() {
    let old = profile();
    let old_source = identity(&old);
    let old_entry = entry(&old);
    let first = NativeColorSourceCatalog::from_revisions([Arc::clone(&old_entry)]).unwrap();
    let original_model = first.resolve(&old_source).unwrap();
    let mut replacement = old.clone();
    replacement.revision += 1;
    let OpticalSource::Additive { emitters } =
        &mut replacement.modes[0].color_physical.as_mut().unwrap().paths[0].source
    else {
        unreachable!()
    };
    emitters[0].xyz = Some(Xyz {
        x: 0.7,
        y: 0.8,
        z: 0.9,
    });
    let catalogue = Arc::new(
        NativeColorSourceCatalog::from_revisions([old_entry, entry(&replacement)]).unwrap(),
    );
    let snapshot = crate::EngineSnapshot {
        native_color_sources: Arc::clone(&catalogue),
        ..Default::default()
    };
    assert!(snapshot.fixtures.is_empty());
    let retained = snapshot.native_color_sources.resolve(&old_source).unwrap();
    assert!(Arc::ptr_eq(&original_model, &retained));
    assert_eq!(
        retained
            .predict(&recipe(&old))
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .x,
        0.1
    );
    assert_eq!(
        catalogue
            .resolve(&identity(&replacement))
            .unwrap()
            .predict(&recipe(&replacement))
            .unwrap()
            .visible
            .unwrap()
            .xyz
            .x,
        0.7
    );
    let changes: [fn(&mut NativeColorIdentity); 4] = [
        |source| source.profile_digest = "not the original".into(),
        |source| source.native_layout_signature = "same labels are insufficient".into(),
        |source| source.model_revision += 1,
        |source| source.path_id = Uuid::new_v4(),
    ];
    for change in changes {
        let mut invalid = old_source.clone();
        change(&mut invalid);
        assert!(catalogue.resolve(&invalid).is_err());
    }
    let resolver: &dyn DynamicNativeModelResolver = catalogue.as_ref();
    assert!(Arc::ptr_eq(
        &retained,
        &resolver.resolve(&old_source).unwrap()
    ));
}

#[test]
fn incremental_entries_reuse_before_decode_and_digest_changes_do_not_reuse() {
    let p = profile();
    let revision = entry(&p);
    let catalog = NativeColorSourceCatalog::from_revisions([Arc::clone(&revision)]).unwrap();
    let reused =
        NativeColorSourceCatalog::compile_revision(revision.key().clone(), Some(&catalog), || {
            panic!("an immutable unchanged revision must not be decoded again")
        });
    assert!(Arc::ptr_eq(&revision, &reused));
    let changed = NativeColorSourceCatalog::compile_revision(
        key(&p, "changed raw digest"),
        Some(&catalog),
        || Ok(p.clone()),
    );
    assert!(!Arc::ptr_eq(&revision, &changed));
    assert!(NativeColorSourceCatalog::from_revisions([Arc::clone(&revision), changed]).is_err());
    assert!(
        NativeColorSourceCatalog::from_revisions([revision, reused])
            .unwrap()
            .is_prepared()
    );
}

#[test]
fn unused_bad_revision_is_cached_without_stopping_available_sources() {
    let p = profile();
    let mut unavailable_key = key(&p, "unreadable retained JSON");
    unavailable_key.revision += 1;
    let unavailable =
        NativeColorSourceCatalog::compile_revision(unavailable_key.clone(), None, || {
            Err("invalid retained optical profile".into())
        });
    assert_eq!(
        unavailable.unavailable_reason(),
        Some("invalid retained optical profile")
    );
    let catalogue =
        NativeColorSourceCatalog::from_revisions([entry(&p), Arc::clone(&unavailable)]).unwrap();
    assert!(catalogue.resolve(&identity(&p)).is_ok());
    let mut missing = identity(&p);
    missing.profile_revision += 1;
    assert!(
        catalogue
            .resolve(&missing)
            .err()
            .unwrap()
            .0
            .contains("invalid retained optical profile")
    );
    let reused =
        NativeColorSourceCatalog::compile_revision(unavailable_key, Some(&catalogue), || {
            panic!("known unavailable immutable revision must also be reused")
        });
    assert!(Arc::ptr_eq(&unavailable, &reused));
}

#[test]
fn decoded_revision_must_match_the_retained_record_key() {
    let p = profile();
    let mut wrong = key(&p, "record digest");
    wrong.profile_id = FixtureId::new();
    let entry = NativeColorSourceCatalog::compile_revision(wrong, None, || Ok(p));
    assert!(entry.unavailable_reason().unwrap().contains("revision key"));
    assert_eq!(entry.source_identities().count(), 0);
}

#[test]
fn unsupported_path_capacity_does_not_discard_other_models_in_that_revision() {
    let mut p = profile();
    let mut unsupported = p.modes[0].clone();
    unsupported.id = Uuid::new_v4();
    unsupported.name = "Too many recordable Color controls".into();
    unsupported.splits = vec![
        FixtureSplit {
            number: 1,
            footprint: 512,
        },
        FixtureSplit {
            number: 2,
            footprint: 1,
        },
    ];
    let channel = unsupported.channels[0].clone();
    unsupported.channels = (0..513)
        .map(|index| {
            let mut channel = channel.clone();
            channel.id = Uuid::new_v4();
            channel.split = if index < 512 { 1 } else { 2 };
            channel
        })
        .collect();
    let path = &mut unsupported.color_physical.as_mut().unwrap().paths[0];
    path.controls = unsupported
        .channels
        .iter()
        .map(|channel| channel.id)
        .collect();
    path.source = OpticalSource::Unknown;
    let unsupported_mode = unsupported.id;
    p.modes.push(unsupported);
    p.validate().unwrap();
    let source = identity(&p);
    let unsupported_source = p
        .native_color_identity(unsupported_mode, p.modes[1].heads[0].id)
        .unwrap();
    let revision = entry(&p);
    assert_eq!(revision.unavailable_reason(), None);
    assert_eq!(revision.source_identities().count(), 2);
    let catalogue = NativeColorSourceCatalog::from_revisions([revision]).unwrap();
    assert!(catalogue.resolve(&source).is_ok());
    assert!(
        catalogue
            .resolve(&unsupported_source)
            .err()
            .unwrap()
            .0
            .contains("1-512 controls")
    );
}

#[test]
fn a_nonrecordable_head_does_not_disable_its_supported_sibling() {
    let mut p = profile();
    let mode = &mut p.modes[0];
    let mut head = mode.heads[0].clone();
    head.id = Uuid::new_v4();
    head.name = "Fixed source without native Color controls".into();
    head.master_shared = false;
    let unsupported_head = head.id;
    mode.heads.push(head);
    mode.color_physical
        .as_mut()
        .unwrap()
        .paths
        .push(HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id: unsupported_head,
            controls: vec![],
            source: OpticalSource::Fixed {
                xyz: Some(Xyz {
                    x: 1.,
                    y: 1.,
                    z: 1.,
                }),
                spectrum: vec![],
                provenance: OpticalProvenance::default(),
            },
            filters: vec![],
            measurements: vec![],
        });
    p.validate().unwrap();
    let supported = identity(&p);
    let unsupported = p
        .native_color_identity(p.modes[0].id, unsupported_head)
        .unwrap();
    let catalogue = NativeColorSourceCatalog::from_revisions([entry(&p)]).unwrap();
    assert!(catalogue.resolve(&supported).is_ok());
    assert!(
        catalogue
            .resolve(&unsupported)
            .err()
            .unwrap()
            .0
            .contains("1-512 controls")
    );
}

#[test]
fn snapshot_json_cannot_restore_or_inject_a_verified_runtime_catalogue() {
    let p = profile();
    let source = identity(&p);
    let snapshot = crate::EngineSnapshot {
        native_color_sources: Arc::new(
            NativeColorSourceCatalog::from_revisions([entry(&p)]).unwrap(),
        ),
        ..Default::default()
    };
    let mut json = serde_json::to_value(&snapshot).unwrap();
    assert!(json.get("native_color_sources").is_none());
    json["native_color_sources"] = serde_json::json!({ "prepared": true });
    let restored: crate::EngineSnapshot = serde_json::from_value(json).unwrap();
    assert!(!restored.native_color_sources.is_prepared());
    assert!(
        restored
            .native_color_sources
            .resolve(&source)
            .err()
            .unwrap()
            .0
            .contains("not prepared")
    );
    assert!(snapshot.native_color_sources.resolve(&source).is_ok());
    let empty = NativeColorSourceCatalog::from_revisions([]).unwrap();
    assert!(empty.is_prepared());
    assert!(
        empty
            .resolve(&source)
            .err()
            .unwrap()
            .0
            .contains("revision is unavailable")
    );
}

mod direct_capture {
    //! TL-595: capture binds one coherent frame to its generation's retained originals.
    use super::*;
    use crate::{DirectColorObservation, Engine, EngineSnapshot};
    use light_core::{
        SessionId,
        programming::{
            DirectCompatibility, DirectIncompatibility, DirectReplay, NativeDriveLimit, UvFallback,
            VisibleFallback,
        },
    };
    use light_fixture::direct_color_samples::{
        SAMPLE_EMITTER_XYZ, direct_color_samples, sample_identity, sample_observation,
    };
    use light_programmer::ProgrammerRegistry;

    fn engine(profiles: &[&FixtureProfile]) -> Engine {
        let registry = ProgrammerRegistry::default();
        registry.start(SessionId::new());
        let engine = Engine::new(registry);
        let catalogue =
            NativeColorSourceCatalog::from_revisions(profiles.iter().map(|p| entry(p))).unwrap();
        engine
            .replace_snapshot(EngineSnapshot {
                native_color_sources: Arc::new(catalogue),
                ..Default::default()
            })
            .unwrap();
        engine
    }

    fn observe(
        token: &crate::CapturedFrameToken,
        target: FixtureId,
        profile: &FixtureProfile,
        raws: [u32; 4],
    ) -> DirectColorObservation {
        DirectColorObservation {
            token: token.clone(),
            target,
            native: sample_observation(profile, raws),
        }
    }

    #[test]
    fn frame_capture_uses_the_pinned_original_and_one_coherent_token() {
        let samples = direct_color_samples();
        // The catalogue also retains the newer revision; the observation pins revision 1.
        let engine = engine(&[&samples.source, &samples.compatible]);
        let frame = engine.prepare_output_frame(Default::default());
        let token = frame.frame_token();
        let (a, b) = (FixtureId::new(), FixtureId::new());
        let captured = frame
            .capture_direct_color(vec![
                observe(&token, a, &samples.source, [255, 0, 0, 0]),
                observe(&token, b, &samples.source, [0, 0, 0, u32::MAX]),
            ])
            .unwrap();
        assert_eq!(captured.len(), 2);
        assert!(captured.iter().all(|c| c.token == token));
        assert_eq!(
            captured[0].capture.portable().visible.unwrap().xyz,
            SAMPLE_EMITTER_XYZ[0],
            "revision 1 appearance, not the recalibrated library revision"
        );
        assert_eq!(
            captured[0].capture.recipe().source,
            sample_identity(&samples.source)
        );
        assert_eq!(
            captured[1].capture.drive_limit(),
            NativeDriveLimit::AboveModelMaximum
        );
        assert!(
            captured[1]
                .capture
                .recipe()
                .channels
                .iter()
                .any(|v| v.raw == u32::MAX)
        );

        let other = engine.prepare_output_frame(Default::default());
        let mixed = frame.capture_direct_color(vec![
            observe(&token, a, &samples.source, [1, 0, 0, 0]),
            observe(&other.frame_token(), b, &samples.source, [1, 0, 0, 0]),
        ]);
        assert!(mixed.is_err(), "no join across captures");
        let foreign = frame.capture_direct_color(vec![observe(
            &other.frame_token(),
            a,
            &samples.source,
            [1, 0, 0, 0],
        )]);
        assert!(foreign.is_err(), "a token must name this capture");
        let duplicate = frame.capture_direct_color(vec![
            observe(&token, a, &samples.source, [1, 0, 0, 0]),
            observe(&token, a, &samples.source, [2, 0, 0, 0]),
        ]);
        assert!(duplicate.is_err());
        let partial = frame.capture_direct_color(vec![
            observe(&token, a, &samples.source, [1, 0, 0, 0]),
            observe(&token, b, &samples.source, [256, 0, 0, 0]),
        ]);
        assert!(partial.is_err(), "all-or-nothing");
        let unretained =
            frame.capture_direct_color(vec![observe(&token, a, &samples.lookalike, [1, 0, 0, 0])]);
        assert!(
            unretained.is_err(),
            "an unretained original cannot verify a capture"
        );
        assert!(frame.capture_direct_color(vec![]).unwrap().is_empty());
        // One Preload branch of this same capture is a coherent lane of its own.
        let state = crate::PreloadFrameState::default();
        let preload = engine
            .prepare_preload_frame(&frame, None)
            .frame_token(&state, crate::PreloadBranch::AfterRelease);
        let branch = frame
            .capture_direct_color(vec![observe(&preload, a, &samples.source, [3, 0, 0, 0])])
            .unwrap();
        assert_eq!(branch[0].token, preload);
        assert!(
            frame
                .capture_direct_color(vec![
                    observe(&preload, a, &samples.source, [3, 0, 0, 0]),
                    observe(&token, b, &samples.source, [3, 0, 0, 0]),
                ])
                .is_err(),
            "Live and Preload lanes are not joined"
        );
    }

    #[test]
    fn a_derived_colour_model_is_a_native_source_that_replays_exactly_on_its_own_layout() {
        // G7a: a mode without an authored physical model is captured and replayed through the
        // derived model the desk outputs through, never only through the portable estimate.
        let samples = direct_color_samples();
        let derived = |profile: &FixtureProfile| {
            let mut profile = profile.clone();
            profile.modes[0].color_physical = None;
            assert!(
                profile.modes[0].derived_color_physical().is_some(),
                "the sample's colour channels derive a model"
            );
            profile
        };
        let source = derived(&samples.source);
        let compatible = derived(&samples.compatible);
        let lookalike = derived(&samples.lookalike);
        let catalogue =
            NativeColorSourceCatalog::from_revisions([&source, &compatible, &lookalike].map(entry))
                .unwrap();
        let projection = source
            .native_color_source(source.modes[0].id)
            .unwrap()
            .into_owned();
        // The identity names the runtime projection, also when read from that projection.
        assert_eq!(sample_identity(&source), sample_identity(&projection));
        let program = catalogue
            .capture_direct(sample_observation(&projection, [9, 999, 0, 0]))
            .expect("a derived model's source is retained and captures")
            .program()
            .clone();
        let plan = |destination: &FixtureProfile| {
            catalogue
                .plan_direct_replay(&program, Some(&sample_identity(destination)))
                .unwrap()
        };
        assert!(matches!(plan(&compatible), DirectReplay::Exact { .. }));
        let DirectReplay::Fallback { compatibility, .. } = plan(&lookalike) else {
            panic!("a different source falls back")
        };
        assert_eq!(
            compatibility,
            DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource)
        );

        // Reopen: a show retains the stored profile JSON, without the derived model. The
        // catalogue compiled from it again names and resolves the same derived source.
        let retained = serde_json::to_value(&source).unwrap();
        assert!(retained["modes"][0]["color_physical"].is_null());
        let reopened =
            NativeColorSourceCatalog::from_revisions([NativeColorSourceCatalog::compile_revision(
                key(&source, "reopened"),
                None,
                || serde_json::from_value(retained).map_err(|error| error.to_string()),
            )])
            .unwrap();
        assert!(matches!(
            reopened
                .resolve_capability(&sample_identity(&source))
                .unwrap(),
            light_dynamics::NativeColorModelCapability::Available(_)
        ));
    }

    #[test]
    fn catalogue_replay_plans_compatible_incompatible_and_unknown_destinations() {
        let samples = direct_color_samples();
        let catalogue = NativeColorSourceCatalog::from_revisions(
            [
                &samples.source,
                &samples.compatible,
                &samples.changed_layout,
                &samples.lookalike,
            ]
            .map(entry),
        )
        .unwrap();
        assert!(
            catalogue
                .capture_direct(sample_observation(&samples.unknown_leakage, [0, 0, 0, 1]))
                .is_err(),
            "an unretained source cannot be captured"
        );
        let program = catalogue
            .capture_direct(sample_observation(&samples.source, [9, 999, 0, 0]))
            .unwrap()
            .program()
            .clone();
        let plan = |destination: Option<NativeColorIdentity>| {
            catalogue
                .plan_direct_replay(&program, destination.as_ref())
                .unwrap()
        };
        assert!(matches!(
            plan(Some(sample_identity(&samples.compatible))),
            DirectReplay::Exact { .. }
        ));
        for (profile, reason) in [
            (
                &samples.changed_layout,
                DirectIncompatibility::ChangedLayout,
            ),
            (&samples.lookalike, DirectIncompatibility::DifferentSource),
        ] {
            let DirectReplay::Fallback { compatibility, .. } = plan(Some(sample_identity(profile)))
            else {
                panic!("{reason:?}")
            };
            assert_eq!(compatibility, DirectCompatibility::Incompatible(reason));
        }
        let DirectReplay::Fallback {
            compatibility,
            fallback,
        } = plan(Some(sample_identity(&samples.unknown_leakage)))
        else {
            panic!("unretained destination model")
        };
        assert!(matches!(compatibility, DirectCompatibility::Unknown(_)));
        assert!(matches!(fallback.visible, VisibleFallback::Fit(_)));
        assert!(matches!(fallback.uv, UvFallback::Apply(uv) if uv.amount == 0.0));
        let DirectReplay::Fallback { compatibility, .. } = plan(None) else {
            panic!("no native Color")
        };
        assert_eq!(
            compatibility,
            DirectCompatibility::Incompatible(DirectIncompatibility::NoNativeColor)
        );
    }
}
