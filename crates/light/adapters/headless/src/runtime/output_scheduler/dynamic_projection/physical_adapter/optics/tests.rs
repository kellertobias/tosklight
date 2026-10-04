//! TL-558 reference cases: the Focus/Zoom adapters resolve against a real captured frame (engine
//! capture, scalar static token, final geometry, token-bound native raw values). Their writes are
//! encoded into DMX bytes, decoded and re-simulated through an independently compiled forward
//! model, which must reproduce the published achieved value.
use super::profiles::*;
use super::*;
use light_core::programming::ZoomIntent;
use light_core::{AttributeKey, ManualClock, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, RenderOptions};
use light_fixture::forward::CompiledOpticsForward;
use light_fixture::{ChannelResolution, FixtureProfile};
use light_programmer::ProgrammerRegistry;
use std::cell::RefCell;

mod continuity;
mod derived_zoom;

pub(in crate::runtime) fn zoom(degrees: f32, convention: OpeningConvention) -> AttributeValue {
    AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention,
    }))
}

pub(in crate::runtime) fn field(degrees: f32) -> AttributeValue {
    zoom(degrees, OpeningConvention::Field)
}

pub(in crate::runtime) fn focus(normalized: f32) -> AttributeValue {
    AttributeValue::Normalized(normalized)
}

struct Rig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    focus: OpticsAdapter,
    zoom: OpticsAdapter,
    clock: Arc<ManualClock>,
    profile: RefCell<FixtureProfile>,
}

/// One resolved head plus the evidence needed to re-simulate it.
struct Resolved {
    descriptor: OpticsDescriptor,
    native: Vec<u32>,
    result: PhysicalResolution<OpticsAdapter>,
}

impl Rig {
    fn new(profile: &FixtureProfile) -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        programmers.start(session);
        let (focus, zoom) = OpticsAdapter::pair();
        let rig = Self {
            engine: Engine::new(programmers.clone()),
            programmers,
            session,
            target: FixtureId::new(),
            focus,
            zoom,
            clock,
            profile: RefCell::new(profile.clone()),
        };
        rig.install(profile);
        rig
    }

    fn install(&self, profile: &FixtureProfile) {
        *self.profile.borrow_mut() = profile.clone();
        self.engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(profile, self.target, 1)].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
    }

    /// A legacy scalar value that seeds the pre-master native baseline.
    fn set(&self, attribute: &str, value: f32) {
        self.programmers.set(
            self.session,
            self.target,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        );
    }

    /// An exact native raw that seeds the pre-master baseline (function selection).
    fn set_raw(&self, attribute: &str, raw: u32) {
        self.programmers.set(
            self.session,
            self.target,
            AttributeKey(attribute.into()),
            AttributeValue::RawDmxExact(raw),
        );
    }

    fn adapter(&self, owner: ProgrammingOwner) -> &OpticsAdapter {
        match owner {
            ProgrammingOwner::Focus => &self.focus,
            _ => &self.zoom,
        }
    }

    fn resolve_with(
        &self,
        owner: ProgrammingOwner,
        value: &AttributeValue,
        previous: Option<&OpticsContinuity>,
        options: RenderOptions,
    ) -> Result<Resolved, TransitionError> {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(options);
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let frame = HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        };
        let adapter = self.adapter(owner);
        let descriptor = adapter
            .compile(&capture.snapshot(), self.target)?
            .expect("optics destination");
        let native = scalar
            .native_raw(&capture, &token, self.target)
            .unwrap()
            .raw()
            .to_vec();
        let result = adapter.resolve(PhysicalRequest {
            frame,
            target: self.target,
            owner,
            descriptor: &descriptor,
            value,
            previous,
        })?;
        Ok(Resolved {
            descriptor,
            native,
            result,
        })
    }

    /// Current adoption of whatever the programmer holds under `zoom`, for a Zoom address in
    /// `convention`, against a real captured frame.
    fn adopt(&self, convention: OpeningConvention) -> Result<AttributeValue, TransitionError> {
        self.clock.advance_millis(25);
        let capture = self.engine.prepare_output_frame(RenderOptions::default());
        let token = capture.frame_token();
        let mut scalar = self.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = self
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let frame = HybridFrameContext {
            capture: &capture,
            geometry: &geometry,
            native_models: models.as_ref(),
            token: &token,
            scalar: &scalar,
        };
        let descriptor = self
            .zoom
            .compile(&capture.snapshot(), self.target)?
            .expect("optics destination");
        let key = ProgrammingOwner::Zoom.key();
        let original = scalar
            .value(self.target, &key)
            .cloned()
            .expect("zoom value");
        let address = DynamicValueAddress {
            representation: light_dynamics::DynamicFamilyRepresentation::Zoom { convention },
            component: Some(light_core::programming::ProgrammingComponent::Zoom),
        };
        self.zoom
            .adopt(frame, &descriptor, self.target, &original, &address)
    }

    fn resolve(&self, owner: ProgrammingOwner, value: &AttributeValue) -> Resolved {
        let resolved = self
            .resolve_with(owner, value, None, RenderOptions::default())
            .unwrap();
        resolved.verify(value, &self.profile.borrow());
        resolved
    }

    fn zoom(&self, value: &AttributeValue) -> Resolved {
        self.resolve(ProgrammingOwner::Zoom, value)
    }

    fn focus(&self, value: &AttributeValue) -> Resolved {
        self.resolve(ProgrammingOwner::Focus, value)
    }
}

/// Decode one fixture's DMX bytes with the builder's sequential layout (fine bytes follow).
fn decode(mode: &light_fixture::FixtureMode, bytes: &[u8; 512]) -> Vec<u32> {
    let mut slot = 1usize;
    mode.channels
        .iter()
        .map(|channel| {
            let mut raw = u32::from(bytes[slot - 1]);
            for secondary in &channel.secondary_slots {
                raw = (raw << 8) | u32::from(bytes[usize::from(*secondary) - 1]);
            }
            slot += channel.resolution.bytes();
            raw
        })
        .collect()
}

impl Resolved {
    fn write(&self) -> NativeControlWrite {
        let [write] = self.result.writes.as_slice() else {
            panic!("exactly the owning control is written")
        };
        *write
    }

    fn raw(&self) -> u32 {
        self.write().raw
    }

    fn requested(&self) -> f64 {
        self.result.requested.value
    }

    fn achieved(&self) -> f64 {
        self.result.achieved.expect("known achieved value")
    }

    /// Complete sidecar checks, then encode the full native output into DMX, decode the bytes
    /// and re-run an independently compiled forward model: it must reproduce `achieved`.
    fn verify(&self, value: &AttributeValue, profile: &FixtureProfile) {
        let (result, descriptor) = (&self.result, &self.descriptor);
        validate_complete_writes(&descriptor.footprint, &result.writes).unwrap();
        let stored = requested(descriptor.family, value).unwrap();
        assert_eq!(result.requested, stored, "the request is never rewritten");
        assert!(
            result.writes.iter().all(|w| w.slot.channel_index != 0),
            "Intensity is never an optics write"
        );
        let mut output = self.native.clone();
        for write in &result.writes {
            assert_eq!(write.slot.destination, descriptor.destination);
            output[write.slot.channel_index as usize] = write.raw;
        }
        let mode = &profile.modes[0];
        let plan = mode.compile_encoding_plan().unwrap();
        let mut bytes = [0u8; 512];
        let values: Vec<_> = (0u32..).zip(output.iter().copied()).collect();
        plan.encode_split_by_index(&mut bytes, 1, 1, &values)
            .unwrap();
        let decoded = decode(mode, &bytes);
        assert_eq!(
            decoded, output,
            "encoded bytes decode to the written values"
        );
        let model = CompiledOpticsForward::compile(mode).unwrap();
        let mut forward = model.create_output();
        model.evaluate(&decoded, &mut forward).unwrap();
        let (achieved, function, quality, nominal, convention) =
            forwarded(descriptor.family, &forward[descriptor.head]);
        assert_eq!(result.achieved, achieved, "achieved is the encoded output");
        assert_eq!(result.quality.function_id, function);
        assert_eq!(result.quality.data_quality, quality);
        assert_eq!(result.quality.nominal, nominal);
        assert_eq!(result.quality.convention, convention);
        if let Some(write) = result.writes.first().filter(|w| !w.parked) {
            assert_eq!(write.function_id, function, "fitted inside its function");
        }
    }
}

#[test]
fn one_zoom_intent_reaches_the_requested_opening_on_differing_optics() {
    let a = Rig::new(&wash_a());
    let b = Rig::new(&spot_b());
    for (rig, step) in [(&a, 30. / 32768.), (&b, 20. / 127.)] {
        let resolved = rig.zoom(&field(20.));
        assert_eq!(resolved.result.quality.status, OpticsFitStatus::Fitted);
        assert!((resolved.achieved() - 20.).abs() <= 0.5 * step + 1e-9);
        assert_eq!(
            resolved.result.quality.convention,
            Some(OpeningConvention::Field)
        );
        assert_eq!(
            resolved.result.quality.data_quality,
            Some(PhysicalDataQuality::Measured)
        );
        assert!(!resolved.result.quality.clipped && !resolved.result.quality.held);
        let resolved = rig.zoom(&field(30.));
        assert!((resolved.achieved() - 30.).abs() <= 0.5 * step + 1e-9);
    }
    // Same degrees, differing native values: reversed U16 vs ascending U8, nonlinear curves.
    assert_eq!(a.zoom(&field(20.)).raw(), 32768);
    assert_eq!(b.zoom(&field(20.)).raw(), 128);
    // Wash A: 50 → 20 over the first half, 20 → 5 over the second (not a straight line).
    assert_eq!(a.zoom(&field(35.)).raw(), 16384);
    assert_eq!(a.zoom(&field(12.5)).raw(), 49152);

    // Clipping publishes the reachable limit and keeps the stored request.
    for (rig, wide, narrow, wide_raw, narrow_raw) in
        [(&a, 50., 5., 0, 65535), (&b, 40., 8., 255, 0)]
    {
        let clipped = rig.zoom(&field(70.));
        assert!(clipped.result.quality.clipped);
        assert_eq!(clipped.result.quality.status, OpticsFitStatus::Fitted);
        assert_eq!((clipped.requested(), clipped.achieved()), (70., wide));
        assert_eq!(clipped.raw(), wide_raw);
        let clipped = rig.zoom(&field(2.));
        assert!(clipped.result.quality.clipped);
        assert_eq!((clipped.requested(), clipped.achieved()), (2., narrow));
        assert_eq!(clipped.raw(), narrow_raw);
    }
    let counters = a.zoom.counters();
    assert_eq!(counters.fitting_compiles, 1, "one fitter per fixture list");
    assert_eq!(counters.clipped, 2);
}

#[test]
fn every_resolution_and_direction_round_trips_through_encoded_dmx_and_forward_simulation() {
    use ChannelResolution::*;
    for resolution in [U8, U16, U24, U32] {
        for descending in [false, true] {
            let (profile, (from, to)) = swept_zoom(resolution, descending);
            let rig = Rig::new(&profile);
            for i in 0..=20 {
                let requested = 4. + i as f32 * 3.2;
                let resolved = rig.zoom(&zoom(requested, OpeningConvention::Beam));
                let raw = resolved.raw();
                assert!(
                    (from..=to).contains(&raw),
                    "{resolution:?} stays in function"
                );
                assert_eq!(
                    resolved.result.quality.clipped,
                    !(8. ..=60.).contains(&requested),
                    "{resolution:?} {requested}"
                );
                assert_eq!(resolved.requested(), f64::from(requested));
                let bound = f64::from(requested).clamp(8., 60.);
                let third = f64::from((to - from) / 3);
                assert!(
                    (resolved.achieved() - bound).abs() <= 0.5 * 40. / third + 1e-9,
                    "{resolution:?} {requested} -> {}",
                    resolved.achieved()
                );
            }
        }
    }
}

#[test]
fn focus_and_zoom_are_independent_owners_with_disjoint_footprints() {
    let rig = Rig::new(&wash_a());
    // Seed both native controls through legacy scalars so a leak would be visible.
    rig.set("zoom", 0.25);
    rig.set("focus", 0.5);
    let zoomed = rig.zoom(&field(20.));
    let focused = rig.focus(&focus(0.25));
    assert_eq!(zoomed.write().slot.channel_index, 1);
    assert_eq!(focused.write().slot.channel_index, 2);
    assert_eq!(zoomed.descriptor.footprint.len(), 1);
    assert_eq!(focused.descriptor.footprint.len(), 1);
    assert_ne!(zoomed.descriptor.footprint, focused.descriptor.footprint);
    // Focus 25% on a reversed 100% → 0% curve over 10..=200.
    assert_eq!(focused.raw(), 153);
    assert!((focused.achieved() - 0.25).abs() <= 0.5 / 190. + 1e-9);
    assert!(!focused.result.quality.nominal, "measured focus travel");
    // The unrequested family keeps its native baseline; nothing is released or zeroed.
    assert_eq!(zoomed.native[2], focused.native[2]);
    assert_eq!(focused.native[1], zoomed.native[1]);
    assert_ne!(
        zoomed.native[2], 0,
        "Focus baseline is present and untouched"
    );
    // Changing one family changes only its own write.
    let wider = rig.zoom(&field(40.));
    assert_ne!(wider.raw(), zoomed.raw());
    assert_eq!(rig.focus(&focus(0.25)).raw(), focused.raw());
    // Each adapter refuses the other family's owner and value.
    assert!(rig.focus.owns(ProgrammingOwner::Focus) && !rig.focus.owns(ProgrammingOwner::Zoom));
    assert!(rig.zoom.owns(ProgrammingOwner::Zoom) && !rig.zoom.owns(ProgrammingOwner::Focus));
    assert!(
        rig.resolve_with(
            ProgrammingOwner::Zoom,
            &focus(0.5),
            None,
            Default::default()
        )
        .is_err()
    );
    let counters = rig.focus.counters();
    assert_eq!(
        (counters.fitting_compiles, counters.descriptor_compiles),
        (1, 5),
        "the Focus/Zoom pair shares one fitter per fixture"
    );
}

#[test]
fn beam_field_convention_is_checked_and_never_converted() {
    let rig = Rig::new(&wash_a());
    rig.set("zoom", 0.5);
    let beam = rig.zoom(&zoom(20., OpeningConvention::Beam));
    let quality = beam.result.quality;
    assert_eq!(quality.status, OpticsFitStatus::ConventionMismatch);
    assert!(quality.held && beam.write().parked);
    assert_eq!(
        beam.raw(),
        beam.native[1],
        "the current output is held, not converted"
    );
    assert_eq!(
        beam.result.requested.convention,
        Some(OpeningConvention::Beam)
    );
    assert_eq!(beam.requested(), 20.);
    // The achieved value describes the held output in the profile's own (Field) convention.
    assert_eq!(quality.convention, Some(OpeningConvention::Field));
    assert!(beam.result.achieved.is_some());

    // A profile without a recorded convention cannot claim either one.
    let mut curve = Curve::zoom(0, 255, (8., 40.), &[]);
    curve.convention = None;
    let rig = Rig::new(
        &OpticsBuilder::new("no convention")
            .zoom(ChannelResolution::U8, curve)
            .build(),
    );
    let unknown = rig.zoom(&field(20.));
    assert_eq!(
        unknown.result.quality.status,
        OpticsFitStatus::UnknownConvention
    );
    assert!(unknown.result.quality.held);
    assert_eq!(unknown.result.quality.convention, None);
    assert_eq!(unknown.requested(), 20.);
}

#[test]
fn unknown_mapping_invalid_requests_and_nominal_focus_are_explicit() {
    // Zoom without degree units: no physical mapping, held, achieved unknown (not zero).
    let mut curve = Curve::zoom(0, 255, (8., 40.), &[]);
    curve.unit = None;
    curve.samples = None;
    curve.convention = None;
    let rig = Rig::new(
        &OpticsBuilder::new("unknown zoom")
            .zoom(ChannelResolution::U8, curve)
            .build(),
    );
    rig.set("zoom", 0.4);
    let unknown = rig.zoom(&field(20.));
    assert_eq!(
        unknown.result.quality.status,
        OpticsFitStatus::UnknownPhysicalMapping
    );
    assert!(unknown.result.quality.held);
    assert_eq!(unknown.raw(), unknown.native[1]);
    assert_ne!(unknown.raw(), 0, "held at the current output, never zeroed");
    assert_eq!(unknown.result.achieved, None, "unknown is not zero");
    assert_eq!(unknown.requested(), 20.);

    // 0° is storable but outside the physical (0°, 180°) opening domain.
    let rig = Rig::new(&wash_a());
    rig.set("zoom", 0.5);
    let closed = rig.zoom(&field(0.));
    assert_eq!(
        closed.result.quality.status,
        OpticsFitStatus::InvalidRequest
    );
    assert!(closed.result.quality.held);
    assert_eq!((closed.requested(), closed.raw()), (0., closed.native[1]));

    // Focus endpoints on the measured reversed curve, and nominal travel on Spot B.
    assert_eq!(rig.focus(&focus(1.)).raw(), 10);
    assert_eq!(rig.focus(&focus(0.)).raw(), 200);
    let rig = Rig::new(&spot_b());
    let nominal = rig.focus(&focus(0.25));
    assert!(nominal.result.quality.nominal);
    assert_eq!(
        nominal.result.quality.data_quality,
        Some(PhysicalDataQuality::Estimated)
    );
    assert_eq!(nominal.raw(), 16384);
    assert_eq!(nominal.achieved(), 16384. / 65535.);
}

#[test]
fn multi_function_zoom_follows_the_current_function_and_accepted_continuity() {
    let profile = multi_function_zoom();
    let ids: Vec<_> = profile.modes[0].channels[1]
        .functions
        .iter()
        .map(|f| f.id)
        .collect();
    let rig = Rig::new(&profile);
    rig.set_raw("zoom", 200);
    let wide = rig.zoom(&field(50.));
    assert_eq!(wide.result.quality.function_id, Some(ids[2]));
    assert!((128..=255).contains(&wide.raw()));
    assert!(!wide.result.quality.clipped);

    // A baseline inside the macro range leaves two fittable functions: never guess.
    rig.set_raw("zoom", 110);
    let ambiguous = rig.zoom(&field(30.));
    assert_eq!(ambiguous.result.quality.status, OpticsFitStatus::Ambiguous);
    assert!(ambiguous.result.quality.held);
    assert_eq!(ambiguous.raw(), 110);
    assert_eq!(ambiguous.result.achieved, None, "the macro has no opening");

    // The lane's last accepted write is reused while the authored command is unchanged: an
    // unfittable request (the other convention) holds that write, not the baseline. A fresh
    // native edit is covered by `continuity::a_fresh_native_function_edit_*` (TL-601).
    rig.set_raw("zoom", 200);
    let accepted = rig.zoom(&field(50.));
    let previous = accepted.result.continuity;
    assert_eq!(previous.baseline, Some(200));
    assert_eq!(previous.response, accepted.descriptor.response);
    let beam = zoom(30., OpeningConvention::Beam);
    let kept = rig
        .resolve_with(
            ProgrammingOwner::Zoom,
            &beam,
            Some(&previous),
            Default::default(),
        )
        .unwrap();
    kept.verify(&beam, &rig.profile.borrow());
    assert!(kept.result.quality.held);
    assert_eq!(kept.raw(), accepted.raw());
    assert_eq!(kept.result.quality.function_id, Some(ids[2]));
    assert_eq!(kept.result.continuity.control.unwrap().2, kept.raw());
    // Continuity of another destination or channel is ignored.
    for foreign in [
        OpticsContinuity {
            destination: FixtureId::new(),
            ..previous
        },
        OpticsContinuity {
            control: Some((1, Uuid::new_v4(), accepted.raw())),
            ..previous
        },
    ] {
        let resolved = rig
            .resolve_with(
                ProgrammingOwner::Zoom,
                &beam,
                Some(&foreign),
                Default::default(),
            )
            .unwrap();
        assert_eq!(resolved.raw(), 200, "held at the authored command");
    }
}

#[test]
fn a_channel_carrying_both_families_publishes_the_conflict_and_writes_nothing() {
    let rig = Rig::new(&shared_focus_zoom_channel());
    for (owner, value) in [
        (ProgrammingOwner::Focus, focus(0.5)),
        (ProgrammingOwner::Zoom, field(20.)),
    ] {
        let resolved = rig.resolve(owner, &value);
        assert_eq!(
            resolved.descriptor.control,
            Err(OpticsFitStatus::OwnershipConflict)
        );
        assert!(resolved.descriptor.footprint.is_empty());
        assert!(resolved.result.writes.is_empty(), "no last-writer-wins");
        assert_eq!(
            resolved.result.quality.status,
            OpticsFitStatus::OwnershipConflict
        );
        assert!(resolved.result.requested.value > 0., "request retained");
        assert_eq!(resolved.result.continuity.control, None);
    }
}

#[test]
fn intensity_masters_and_blackout_are_left_to_the_single_final_render() {
    let rig = Rig::new(&wash_a());
    let base = rig.zoom(&field(20.));
    rig.set("intensity", 0.3);
    for options in [
        RenderOptions::default(),
        RenderOptions {
            grand_master: 0.25,
            ..Default::default()
        },
        RenderOptions {
            blackout: true,
            ..Default::default()
        },
    ] {
        let resolved = rig
            .resolve_with(ProgrammingOwner::Zoom, &field(20.), None, options)
            .unwrap();
        resolved.verify(&field(20.), &rig.profile.borrow());
        assert!(resolved.native[0] > 0, "{options:?}: pre-master Intensity");
        assert_eq!(resolved.result.writes, base.result.writes, "{options:?}");
        assert_eq!(resolved.result.achieved, base.result.achieved);
    }
}

#[test]
fn fixture_replacement_recompiles_and_keeps_the_stored_request() {
    let rig = Rig::new(&wash_a());
    let first = rig.zoom(&field(20.));
    rig.install(&spot_b());
    let second = rig
        .resolve_with(
            ProgrammingOwner::Zoom,
            &field(20.),
            Some(&first.result.continuity),
            Default::default(),
        )
        .unwrap();
    second.verify(&field(20.), &rig.profile.borrow());
    assert_eq!(second.result.requested, first.result.requested);
    assert_eq!(second.raw(), 128, "the new optics' own curve");
    assert!((second.achieved() - 20.).abs() < 1e-9);
    assert_eq!(rig.zoom.counters().fitting_compiles, 2);
}

#[test]
fn legacy_zoom_current_adopts_the_measured_opening_or_stays_a_requirement() {
    let requires = Err(TransitionError::Requires(
        TransitionRequirement::ZoomConvention,
    ));
    let rig = Rig::new(&wash_a());
    // Exact raws on the reversed nonlinear U16 curve: 50° → 20° → 5°.
    for (raw, degrees) in [(0, 50.), (16384, 35.), (32768, 20.), (65535, 5.)] {
        rig.set_raw("zoom", raw);
        assert_eq!(
            rig.adopt(OpeningConvention::Field),
            Ok(field(degrees)),
            "raw {raw}"
        );
    }
    // A legacy percentage is measured through its encoded raw, never read as degrees.
    rig.set("zoom", 0.5);
    let AttributeValue::Zoom(adopted) = rig.adopt(OpeningConvention::Field).unwrap() else {
        panic!("Zoom family")
    };
    let ScalarIntent::Value(degrees) = adopted.opening_degrees else {
        panic!("materialized opening")
    };
    assert!((degrees - 20.).abs() < 1e-3, "{degrees}");
    // The other convention is not converted.
    assert_eq!(rig.adopt(OpeningConvention::Beam), requires);
    // A typed Zoom value is not a legacy scalar and is never re-measured here.
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Zoom.key(),
        zoom(30., OpeningConvention::Beam),
    );
    assert_eq!(rig.adopt(OpeningConvention::Field), requires);

    // A raw inside a macro function has no opening.
    let rig = Rig::new(&multi_function_zoom());
    rig.set_raw("zoom", 110);
    assert_eq!(rig.adopt(OpeningConvention::Field), requires);
    rig.set_raw("zoom", 200);
    assert!(rig.adopt(OpeningConvention::Field).is_ok());
    // No recorded convention: it cannot be claimed.
    let mut curve = Curve::zoom(0, 255, (8., 40.), &[]);
    curve.convention = None;
    let rig = Rig::new(
        &OpticsBuilder::new("no convention")
            .zoom(ChannelResolution::U8, curve)
            .build(),
    );
    rig.set_raw("zoom", 128);
    assert_eq!(rig.adopt(OpeningConvention::Field), requires);
}
