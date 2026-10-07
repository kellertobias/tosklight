//! Reference cases for TL-558: fixture-level Focus/Zoom destination fitting.
use super::*;

fn optics_channel(
    head: Uuid,
    family: &str,
    resolution: ChannelResolution,
    secondary: Vec<u16>,
    (from, to): (u32, u32),
    (physical_from, physical_to): (f32, f32),
    unit: &str,
) -> FixtureChannel {
    let mut value = channel(head, resolution, secondary);
    value.attribute = AttributeKey(family.into());
    value.fixture_attribute = value.attribute.clone();
    let function = &mut value.functions[0];
    function.attribute = value.attribute.clone();
    function.dmx_from = from;
    function.dmx_to = to;
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: physical_from,
        physical_max: physical_to,
        unit: Some(unit.into()),
    };
    value
}

fn calibrate(
    function: &mut ChannelFunction,
    quality: PhysicalDataQuality,
    opening_convention: Option<OpeningConvention>,
    samples: &[(u32, f32)],
) {
    function.physical_mapping = Some(PhysicalMappingCalibration {
        quality,
        source: Some("Synthetic reference".into()),
        opening_convention,
        samples: samples
            .iter()
            .map(|&(raw, physical)| PhysicalMappingPoint { raw, physical })
            .collect(),
        ..Default::default()
    });
}

fn mode(
    heads: Vec<FixtureHead>,
    channels: Vec<FixtureChannel>,
    splits: &[(u16, u16)],
) -> FixtureMode {
    let mut mode = FixtureProfile::blank().modes.remove(0);
    mode.heads = heads;
    mode.channels = channels;
    mode.splits = splits
        .iter()
        .map(|&(number, footprint)| FixtureSplit { number, footprint })
        .collect();
    mode
}

fn head(master_shared: bool) -> FixtureHead {
    FixtureHead {
        id: Uuid::new_v4(),
        name: "Head".into(),
        master_shared,
    }
}

/// Intensity (untouched), U16 Field zoom 50° → 20° → 5°, U8 measured Focus 100% → 0% on 10..=200.
fn reference() -> FixtureMode {
    let h = head(true);
    let dimmer = channel(h.id, ChannelResolution::U8, vec![]);
    let mut zoom = optics_channel(
        h.id,
        "zoom",
        ChannelResolution::U16,
        vec![3],
        (0, 65535),
        (50., 5.),
        "deg",
    );
    calibrate(
        &mut zoom.functions[0],
        PhysicalDataQuality::Measured,
        Some(OpeningConvention::Field),
        &[(0, 50.), (32768, 20.), (65535, 5.)],
    );
    let mut focus = optics_channel(
        h.id,
        "focus",
        ChannelResolution::U8,
        vec![],
        (10, 200),
        (100., 0.),
        "%",
    );
    calibrate(
        &mut focus.functions[0],
        PhysicalDataQuality::Measured,
        None,
        &[],
    );
    mode(vec![h], vec![dimmer, zoom, focus], &[(1, 4)])
}

fn zoom(degrees: f64) -> ZoomFitRequest {
    ZoomFitRequest {
        degrees,
        convention: None,
        function_id: None,
    }
}

fn focus(normalized: f64) -> FocusFitRequest {
    FocusFitRequest {
        normalized,
        function_id: None,
    }
}

struct Solver {
    fitting: CompiledOpticsFitting,
    workspace: OpticsFitWorkspace,
    output: Vec<OpticsFitResult>,
}

impl Solver {
    fn new(mode: &FixtureMode) -> Self {
        let fitting = CompiledOpticsFitting::compile(mode).unwrap();
        Self {
            workspace: fitting.create_workspace(),
            output: fitting.create_output(),
            fitting,
        }
    }

    fn fit(&mut self, current: &[u32], requests: &[OpticsFitRequest]) -> &[OpticsFitResult] {
        self.fitting
            .fit(current, requests, &mut self.workspace, &mut self.output)
            .unwrap();
        &self.output
    }

    /// Independent forward evaluation of the proposed native frame.
    fn forward(&self) -> Vec<crate::forward::OpticsForwardResult> {
        let forward = self.fitting.forward();
        let mut out = forward.create_output();
        forward
            .evaluate(self.workspace.proposed_raw(), &mut out)
            .unwrap();
        out
    }
}

fn request(focus: Option<FocusFitRequest>, zoom: Option<ZoomFitRequest>) -> OpticsFitRequest {
    OpticsFitRequest { focus, zoom }
}

#[test]
fn endpoint_interior_and_clipped_requests_round_trip_through_the_forward_model() {
    let m = reference();
    let mut s = Solver::new(&m);
    let current = [77, 1000, 50];
    let cases = [
        (50., 0, 0.5, 105, false),
        (5., 65535, 1., 10, false),
        (20., 32768, 0., 200, false),
        (60., 0, 1.25, 10, true),
        (2., 65535, -0.5, 200, true),
    ];
    for (degrees, zoom_raw, normalized, focus_raw, clipped) in cases {
        let r = s.fit(
            &current,
            &[request(Some(focus(normalized)), Some(zoom(degrees)))],
        )[0];
        assert_eq!(r.zoom.status, OpticsFitStatus::Fitted);
        assert_eq!(r.zoom.write.unwrap().raw, zoom_raw);
        assert_eq!(r.zoom.write.unwrap().channel_index, 1);
        assert_eq!(
            r.zoom.requested,
            Some(degrees),
            "requested degrees are retained"
        );
        assert_eq!(r.zoom.clipped, clipped);
        assert_eq!(r.zoom.convention, Some(OpeningConvention::Field));
        assert_eq!(r.zoom.quality, Some(PhysicalDataQuality::Measured));
        assert_eq!(r.focus.write.unwrap().raw, focus_raw);
        assert_eq!(r.focus.requested, Some(normalized));
        assert_eq!(r.focus.clipped, clipped);
        assert!(!r.focus.nominal);
        let forward = s.forward();
        assert_eq!(forward[0].zoom.unwrap().degrees, r.zoom.achieved.unwrap());
        assert_eq!(
            forward[0].focus.unwrap().percent / 100.,
            r.focus.achieved.unwrap()
        );
        assert_eq!(s.workspace.proposed_raw()[0], 77, "dimmer is untouched");
    }
    let r = s.fit(&current, &[request(None, Some(zoom(12.5)))])[0];
    let write = r.zoom.write.unwrap();
    assert!(write.raw > 32768 && write.raw < 65535);
    assert!((r.zoom.achieved.unwrap() - 12.5).abs() <= 0.5 * 15. / 32767. + 1e-12);
}

fn swept_zoom(resolution: ChannelResolution, descending: bool) -> (FixtureMode, (u32, u32), f64) {
    let h = head(false);
    let secondary = (2..=resolution.bytes() as u16).collect();
    let (from, to) = (3, resolution.max_raw() - 5);
    let third = from + (to - from) / 3;
    let (a, b, c) = if descending {
        (60., 20., 8.)
    } else {
        (8., 20., 60.)
    };
    let mut z = optics_channel(
        h.id,
        "zoom",
        resolution,
        secondary,
        (from, to),
        (a, c),
        "degrees",
    );
    calibrate(
        &mut z.functions[0],
        PhysicalDataQuality::Manufacturer,
        Some(OpeningConvention::Beam),
        &[(from, a), (third, b), (to, c)],
    );
    // Conservative bound on degrees per native step for either direction.
    let steepest = 40. / f64::from((third - from).min(to - third));
    let footprint = resolution.bytes() as u16;
    (
        mode(vec![h], vec![z], &[(1, footprint)]),
        (from, to),
        steepest,
    )
}

fn decode(frame: &[u8; 512], mode: &FixtureMode) -> u32 {
    let c = &mode.channels[0];
    let primary = mode.primary_slots().unwrap()[&c.id];
    std::iter::once(primary)
        .chain(c.secondary_slots.iter().copied())
        .fold(0u32, |raw, slot| {
            (raw << 8) | u32::from(frame[usize::from(slot) - 1])
        })
}

#[test]
fn every_resolution_and_direction_stays_in_bounds_and_encodes_round_trip() {
    use ChannelResolution::*;
    for resolution in [U8, U16, U24, U32] {
        for descending in [false, true] {
            let (m, (from, to), step) = swept_zoom(resolution, descending);
            let plan = m.compile_encoding_plan().unwrap();
            let mut s = Solver::new(&m);
            for i in 0..=40 {
                let requested = 4. + f64::from(i) * 1.6;
                let r = s.fit(&[from], &[request(None, Some(zoom(requested)))])[0].zoom;
                let write = r.write.unwrap();
                assert!(
                    (from..=to).contains(&write.raw),
                    "{resolution:?} stays in function"
                );
                let achieved = r.achieved.unwrap();
                assert_eq!(r.clipped, !(8. ..=60.).contains(&requested));
                assert!(
                    (achieved - requested.clamp(8., 60.)).abs() <= 0.5 * step + 1e-9,
                    "{resolution:?} {requested} -> {achieved}"
                );
                let mut frame = [0; 512];
                plan.encode_split_by_index(
                    &mut frame,
                    1,
                    write.split,
                    &[(write.channel_index, write.raw)],
                )
                .unwrap();
                let decoded = decode(&frame, &m);
                assert_eq!(decoded, write.raw);
                let forward = s.fitting.forward();
                let mut out = forward.create_output();
                forward.evaluate(&[decoded], &mut out).unwrap();
                assert_eq!(out[0].zoom.unwrap().degrees, achieved);
                assert_eq!(
                    out[0].zoom.unwrap().convention,
                    Some(OpeningConvention::Beam)
                );
            }
        }
    }
}

#[test]
fn focus_nominal_travel_has_no_distance_claim_and_zoom_without_degrees_is_unavailable() {
    let mut m = reference();
    m.channels[1].functions[0].physical_mapping = None;
    for c in &mut m.channels[1..] {
        c.functions[0].physical_mapping = None;
        if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut c.functions[0].behavior {
            *unit = Some("m".into());
        }
    }
    let mut s = Solver::new(&m);
    let r = s.fit(
        &[0, 400, 150],
        &[request(Some(focus(0.25)), Some(zoom(20.)))],
    )[0];
    assert_eq!(r.focus.status, OpticsFitStatus::Fitted);
    assert!(r.focus.nominal);
    assert_eq!(r.focus.quality, Some(PhysicalDataQuality::Estimated));
    assert_eq!(r.focus.write.unwrap().raw, 58);
    assert_eq!(r.focus.achieved, Some(48. / 190.));
    assert_eq!(r.zoom.status, OpticsFitStatus::UnknownPhysicalMapping);
    assert_eq!((r.zoom.write, r.zoom.achieved), (None, None));
    assert_eq!(r.zoom.requested, Some(20.));
    assert_eq!(s.workspace.proposed_raw(), &[0, 400, 58]);
    assert_eq!(s.forward()[0].focus.unwrap().percent, 100. * 48. / 190.);
}

#[test]
fn normalized_ascending_focus_calibration_is_measured_travel() {
    let h = head(false);
    let mut f = optics_channel(
        h.id,
        "focus",
        ChannelResolution::U16,
        vec![2],
        (0, 65535),
        (0., 1.),
        "normalized",
    );
    calibrate(
        &mut f.functions[0],
        PhysicalDataQuality::Manufacturer,
        None,
        &[(0, 0.), (16384, 0.5), (65535, 1.)],
    );
    let mut s = Solver::new(&mode(vec![h], vec![f], &[(1, 2)]));
    let r = s.fit(&[9], &[request(Some(focus(0.25)), None)])[0].focus;
    assert_eq!(r.write.unwrap().raw, 8192);
    assert_eq!(r.achieved, Some(0.25));
    assert_eq!(r.quality, Some(PhysicalDataQuality::Manufacturer));
    assert!(!r.nominal);
    assert_eq!(s.forward()[0].focus.unwrap().percent, 25.);
}

#[test]
fn focus_and_zoom_requests_are_independent_and_unrelated_channels_untouched() {
    let m = reference();
    let mut s = Solver::new(&m);
    let current = [200, 1234, 99];
    let r = s.fit(&current, &[request(Some(focus(1.)), None)])[0];
    assert_eq!(r.zoom, OpticsFit::default());
    assert_eq!(r.zoom.status, OpticsFitStatus::NotRequested);
    assert_eq!(s.workspace.proposed_raw(), &[200, 1234, 10]);
    let before = s.forward()[0].zoom;
    let r = s.fit(&current, &[request(None, Some(zoom(50.)))])[0];
    assert_eq!(r.focus.status, OpticsFitStatus::NotRequested);
    assert_eq!(s.workspace.proposed_raw(), &[200, 0, 99]);
    assert!((s.forward()[0].focus.unwrap().percent - 100. * 101. / 190.).abs() < 1e-9);
    assert_ne!(before, s.forward()[0].zoom);
    s.fit(&current, &[OpticsFitRequest::default()]);
    assert_eq!(s.workspace.proposed_raw(), &current);
}

#[test]
fn zoom_convention_is_checked_never_converted_and_unknown_calibration_is_explicit() {
    let mut m = reference();
    let mut s = Solver::new(&m);
    let beam = ZoomFitRequest {
        convention: Some(OpeningConvention::Beam),
        ..zoom(20.)
    };
    let r = s.fit(&[0, 0, 10], &[request(None, Some(beam))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::ConventionMismatch);
    assert_eq!((r.requested, r.achieved, r.write), (Some(20.), None, None));
    let field = ZoomFitRequest {
        convention: Some(OpeningConvention::Field),
        ..zoom(20.)
    };
    assert_eq!(
        s.fit(&[0, 0, 10], &[request(None, Some(field))])[0]
            .zoom
            .status,
        OpticsFitStatus::Fitted
    );
    m.channels[1].functions[0].physical_mapping = None;
    let mut s = Solver::new(&m);
    let r = s.fit(&[0, 0, 10], &[request(None, Some(field))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::UnknownConvention);
    let r = s.fit(&[0, 0, 10], &[request(None, Some(zoom(27.5)))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::Fitted);
    assert_eq!(r.quality, Some(PhysicalDataQuality::Unknown));
    assert_eq!(r.convention, None);
    assert_eq!(r.write.unwrap().raw, 32768);
}

fn multi_function_zoom() -> FixtureMode {
    let h = head(false);
    let mut z = optics_channel(
        h.id,
        "zoom",
        ChannelResolution::U8,
        vec![],
        (0, 99),
        (40., 10.),
        "deg",
    );
    calibrate(
        &mut z.functions[0],
        PhysicalDataQuality::Measured,
        Some(OpeningConvention::Field),
        &[],
    );
    let mut macro_fn = z.functions[0].clone();
    macro_fn.id = Uuid::new_v4();
    macro_fn.dmx_from = 100;
    macro_fn.dmx_to = 127;
    macro_fn.physical_mapping = None;
    macro_fn.behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "zoom_macro".into(),
        label: "Macro".into(),
        raw_value: 100,
    };
    let mut wide = z.functions[0].clone();
    wide.id = Uuid::new_v4();
    wide.dmx_from = 128;
    wide.dmx_to = 255;
    wide.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 20.,
        physical_max: 60.,
        unit: Some("deg".into()),
    };
    z.functions.extend([macro_fn, wide]);
    mode(vec![h], vec![z], &[(1, 1)])
}

#[test]
fn multi_function_controls_use_pinned_or_current_function_and_never_guess() {
    let m = multi_function_zoom();
    let ids: Vec<_> = m.channels[0].functions.iter().map(|f| f.id).collect();
    let mut s = Solver::new(&m);
    let r = s.fit(&[50], &[request(None, Some(zoom(55.)))])[0].zoom;
    assert_eq!(r.function_id, Some(ids[0]), "current function is kept");
    assert_eq!(
        (r.write.unwrap().raw, r.achieved, r.clipped),
        (0, Some(40.), true)
    );
    let r = s.fit(&[200], &[request(None, Some(zoom(55.)))])[0].zoom;
    assert_eq!(r.function_id, Some(ids[2]));
    assert!((128..=255).contains(&r.write.unwrap().raw));
    assert!(!r.clipped);
    let r = s.fit(&[110], &[request(None, Some(zoom(30.)))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::Ambiguous);
    assert_eq!(r.write, None);
    let pinned = |id| ZoomFitRequest {
        function_id: Some(id),
        ..zoom(30.)
    };
    let r = s.fit(&[110], &[request(None, Some(pinned(ids[2])))])[0].zoom;
    assert_eq!(
        (r.status, r.write.unwrap().raw),
        (OpticsFitStatus::Fitted, 160)
    );
    let r = s.fit(&[110], &[request(None, Some(pinned(ids[1])))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::UnknownPhysicalMapping);
    let r = s.fit(&[110], &[request(None, Some(pinned(Uuid::new_v4())))])[0].zoom;
    assert_eq!(r.status, OpticsFitStatus::UnknownFunction);
    assert_eq!(
        s.forward()[0].zoom_status,
        crate::forward::OpticsForwardStatus::UnknownPhysicalMapping
    );
}

fn split_multi_head() -> FixtureMode {
    let mut m = reference();
    let second = head(false);
    let mut own_focus = m.channels[2].clone();
    own_focus.id = Uuid::new_v4();
    own_focus.functions[0].id = Uuid::new_v4();
    own_focus.head_id = second.id;
    own_focus.split = 2;
    m.heads.push(second);
    m.channels.push(own_focus);
    m.splits.push(FixtureSplit {
        number: 2,
        footprint: 1,
    });
    m
}

#[test]
fn split_multi_head_and_master_shared_controls_stay_independent() {
    let m = split_multi_head();
    let mut s = Solver::new(&m);
    let current = [0, 0, 10, 10];
    let out = s.fit(
        &current,
        &[
            request(Some(focus(0.)), Some(zoom(20.))),
            request(Some(focus(1.)), Some(zoom(20.))),
        ],
    );
    assert!(!out[0].zoom.shared);
    assert!(out[1].zoom.shared, "second head inherits the shared zoom");
    assert_eq!(out[0].zoom.write, out[1].zoom.write);
    assert_eq!(out[1].focus.write.unwrap().split, 2);
    assert_eq!(out[1].focus.write.unwrap().channel_index, 3);
    assert!(!out[1].focus.shared);
    assert_eq!(s.workspace.proposed_raw(), &[0, 32768, 200, 10]);
    let out = s.fit(
        &current,
        &[
            request(None, Some(zoom(20.))),
            request(None, Some(zoom(30.))),
        ],
    );
    for r in out {
        assert_eq!(r.zoom.status, OpticsFitStatus::OwnershipConflict);
        assert_eq!(r.zoom.write, None);
    }
    assert_eq!(s.workspace.proposed_raw(), &current);
    let out = s.fit(
        &current,
        &[OpticsFitRequest::default(), request(None, Some(zoom(30.)))],
    );
    assert_eq!(out[1].zoom.status, OpticsFitStatus::Fitted);
    let forward = s.forward();
    assert_eq!(
        forward[0].zoom, forward[1].zoom,
        "a shared write affects every inheriting head"
    );

    let mut ambiguous = m.clone();
    let mut extra = ambiguous.channels[3].clone();
    extra.id = Uuid::new_v4();
    extra.functions[0].id = Uuid::new_v4();
    ambiguous.channels.push(extra);
    ambiguous.splits[1].footprint = 2;
    let mut s = Solver::new(&ambiguous);
    let out = s.fit(&[0, 0, 10, 10, 10], &[request(Some(focus(0.5)), None); 2]);
    assert_eq!(out[0].focus.status, OpticsFitStatus::Fitted);
    assert_eq!(out[1].focus.status, OpticsFitStatus::Ambiguous);
}

#[test]
fn control_accessor_reports_the_owning_footprint_or_its_binding_status() {
    let m = split_multi_head();
    let s = Solver::new(&m);
    let second = s.fitting.head_index(m.heads[1].id).unwrap();
    assert_eq!(second, 1);
    assert_eq!(s.fitting.head_index(Uuid::new_v4()), None);
    let zoom = s.fitting.control(1, OpticsFamily::Zoom).unwrap().unwrap();
    assert_eq!(
        (
            zoom.channel_index,
            zoom.channel_id,
            zoom.split,
            zoom.shared,
            zoom.raw_max
        ),
        (1, m.channels[1].id, 1, true, 65535),
        "the second head inherits the master-shared U16 zoom"
    );
    let focus = s.fitting.control(1, OpticsFamily::Focus).unwrap().unwrap();
    assert_eq!(
        (focus.channel_index, focus.split, focus.shared),
        (3, 2, false)
    );
    assert!(s.fitting.control(2, OpticsFamily::Focus).is_none());
    let mut unsupported = reference();
    unsupported.channels.truncate(2);
    let u = Solver::new(&unsupported);
    assert_eq!(
        u.fitting.control(0, OpticsFamily::Focus),
        Some(Err(OpticsFitStatus::Unsupported))
    );
}

#[test]
fn a_control_carrying_both_families_is_reported_as_ownership_conflict() {
    let h = head(false);
    let mut c = optics_channel(
        h.id,
        "focus",
        ChannelResolution::U8,
        vec![],
        (0, 127),
        (0., 100.),
        "%",
    );
    let mut z = c.functions[0].clone();
    z.id = Uuid::new_v4();
    z.attribute = AttributeKey("zoom".into());
    z.dmx_from = 128;
    z.dmx_to = 255;
    z.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 10.,
        physical_max: 40.,
        unit: Some("deg".into()),
    };
    c.functions.push(z);
    let mut s = Solver::new(&mode(vec![h], vec![c], &[(1, 1)]));
    let r = s.fit(&[0], &[request(Some(focus(0.5)), Some(zoom(20.)))])[0];
    assert_eq!(r.focus.status, OpticsFitStatus::OwnershipConflict);
    assert_eq!(r.zoom.status, OpticsFitStatus::OwnershipConflict);
    assert_eq!(s.workspace.proposed_raw(), &[0]);
    for family in [OpticsFamily::Focus, OpticsFamily::Zoom] {
        assert_eq!(
            s.fitting.control(0, family),
            Some(Err(OpticsFitStatus::OwnershipConflict))
        );
    }
}

#[test]
fn invalid_requests_and_input_layouts_are_rejected_explicitly() {
    let m = reference();
    let mut s = Solver::new(&m);
    for degrees in [0., -5., 180., f64::NAN, f64::INFINITY] {
        let r = s.fit(
            &[0, 0, 10],
            &[request(Some(focus(f64::NAN)), Some(zoom(degrees)))],
        )[0];
        assert_eq!(r.zoom.status, OpticsFitStatus::InvalidRequest);
        assert_eq!(r.focus.status, OpticsFitStatus::InvalidRequest);
    }
    let mut unsupported = m.clone();
    unsupported.channels.truncate(1);
    let mut u = Solver::new(&unsupported);
    let r = u.fit(&[0], &[request(Some(focus(0.5)), Some(zoom(20.)))])[0];
    assert_eq!(
        (r.focus.status, r.zoom.status),
        (OpticsFitStatus::Unsupported, OpticsFitStatus::Unsupported)
    );
    let f = &s.fitting;
    let req = [OpticsFitRequest::default()];
    let (mut ws, mut out) = (f.create_workspace(), f.create_output());
    assert_eq!(
        f.fit(&[0, 0], &req, &mut ws, &mut out),
        Err(OpticsFitInputError::ChannelCount)
    );
    assert_eq!(
        f.fit(&[256, 0, 0], &req, &mut ws, &mut out),
        Err(OpticsFitInputError::RawOutOfRange)
    );
    assert_eq!(
        f.fit(&[0, 0, 0], &[], &mut ws, &mut out),
        Err(OpticsFitInputError::RequestLayout)
    );
    assert_eq!(
        f.fit(&[0, 0, 0], &req, &mut ws, &mut []),
        Err(OpticsFitInputError::OutputLayout)
    );
    let mut other = u.fitting.create_workspace();
    assert_eq!(
        f.fit(&[0, 0, 0], &req, &mut other, &mut out),
        Err(OpticsFitInputError::WorkspaceLayout)
    );
}

#[test]
fn repeated_solves_reuse_caller_buffers_and_are_deterministic() {
    let m = split_multi_head();
    let mut s = Solver::new(&m);
    let raw_ptr = s.workspace.proposed_raw().as_ptr();
    let forward_ptr = s.workspace.forward().as_ptr();
    let output_ptr = s.output.as_ptr();
    let requests = [
        request(Some(focus(0.3)), Some(zoom(33.))),
        request(Some(focus(0.7)), None),
    ];
    let first = s.fit(&[0, 5, 10, 10], &requests).to_vec();
    for i in 0..1000u32 {
        let current = [i % 256, i * 61, 10 + i % 190, 200 - i % 190];
        let out = s.fit(&current, &requests);
        assert_eq!(out[0].zoom.write, first[0].zoom.write);
        assert_eq!(out[1].focus.write, first[1].focus.write);
    }
    assert_eq!(s.workspace.proposed_raw().as_ptr(), raw_ptr);
    assert_eq!(s.workspace.forward().as_ptr(), forward_ptr);
    assert_eq!(s.output.as_ptr(), output_ptr);
}
