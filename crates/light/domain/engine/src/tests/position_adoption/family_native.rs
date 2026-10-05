//! One per-frame native installation for Position, Color (lamp and Media) and Focus/Zoom
//! (TL-548 C2). Synthetic profiles establish footprint, completeness and ownership semantics.
use super::*;
use crate::{FamilyNativeWrite, PreparedStaticFamilyFrame, RenderResult};
use light_core::programming::{PositionIntent, ProgrammingOwner};
use light_fixture::{
    ColorPhysicalModel, HeadOpticalPath, NativeColorBinding, OpticalEmitter, OpticalEmitterBand,
    OpticalProvenance as Provenance, OpticalSource, PhysicalDataQuality,
};

#[path = "family_native_shared.rs"]
mod family_native_shared;

/// Wash channel order: intensity, red, green, blue, zoom, focus.
pub(super) const RED: usize = 1;
pub(super) const ZOOM: usize = 4;
pub(super) const FOCUS: usize = 5;

pub(super) fn copy_at(fixture: &mut PatchedFixture, address: u16) -> Uuid {
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(address),
        ..Default::default()
    });
    copy
}

/// Lamp RGB wash with a compiled Color path, a Zoom and a Focus control, patched at 20 and 30.
/// `color_claims_zoom` lists the Zoom channel
/// as an (unmodeled) Color path control, so two families own one control.
fn wash(color_claims_zoom: bool) -> PatchedFixture {
    let (mut fixture, _) = schema_v2_fixture(&[
        ("intensity", false, false),
        ("color.red", false, false),
        ("color.green", false, false),
        ("color.blue", false, false),
        ("zoom", false, false),
        ("focus", false, false),
    ]);
    fixture.address = Some(20);
    redefine(&mut fixture, |profile| {
        let mode = &mut profile.modes[0];
        let emitters = [RED, RED + 1, RED + 2]
            .into_iter()
            .enumerate()
            .map(|(i, index)| {
                let channel = &mode.channels[index];
                let mut xyz = [0.; 3];
                xyz[i] = 1.;
                OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: channel.attribute.0.to_string(),
                    binding: NativeColorBinding {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                    },
                    xyz: Some(Xyz {
                        x: xyz[0],
                        y: xyz[1],
                        z: xyz[2],
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Visible,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: Provenance {
                        quality: PhysicalDataQuality::Estimated,
                        ..Default::default()
                    },
                }
            })
            .collect();
        let mut controls: Vec<_> = [RED, RED + 1, RED + 2]
            .map(|index| mode.channels[index].id)
            .into();
        if color_claims_zoom {
            controls.push(mode.channels[ZOOM].id);
        }
        mode.color_physical = Some(ColorPhysicalModel {
            version: 1,
            revision: 1,
            paths: vec![HeadOpticalPath {
                id: Uuid::new_v4(),
                head_id: mode.heads[0].id,
                controls,
                source: OpticalSource::Additive { emitters },
                filters: vec![],
                measurements: vec![],
            }],
        });
    });
    copy_at(&mut fixture, 30);
    fixture
}

/// A Media layer head with the plain 8-bit Media color wire contract, patched at 40, no copies.
fn media() -> PatchedFixture {
    let (mut fixture, _) = schema_v2_fixture(
        &[
            "media.layer.cyan",
            "media.layer.magenta",
            "media.layer.yellow",
            "media.layer.grayscale",
        ]
        .map(|name| (name, false, false)),
    );
    fixture.address = Some(40);
    redefine(&mut fixture, |profile| {
        for channel in &mut profile.modes[0].channels {
            channel.functions[0].behavior = light_fixture::ChannelFunctionBehavior::Continuous {
                physical_min: 0.,
                physical_max: 255.,
                unit: None,
            };
        }
    });
    fixture
}

pub(super) struct Rig {
    pub(super) engine: Engine,
    pub(super) mover: PatchedFixture,
    pub(super) wash: PatchedFixture,
    pub(super) media: PatchedFixture,
}

impl Rig {
    pub(super) fn new(color_claims_zoom: bool) -> Self {
        let mut mover = mover();
        copy_at(&mut mover, 10);
        let mut wash = wash(color_claims_zoom);
        wash.fixture_number = Some(2);
        let mut media = media();
        media.fixture_number = Some(3);
        let programmers = ProgrammerRegistry::default();
        let session = SessionId::new();
        programmers.start(session);
        programmers.set(
            session,
            mover.fixture_id,
            ProgrammingOwner::Position.key(),
            AttributeValue::Position(Arc::new(PositionIntent::angles(10., 20.))),
        );
        for target in [wash.fixture_id, media.fixture_id] {
            programmers.set(
                session,
                target,
                AttributeKey::color(),
                AttributeValue::ColorXyz(Xyz {
                    x: 0.3,
                    y: 0.3,
                    z: 0.3,
                }),
            );
        }
        set(&programmers, session, wash.fixture_id, "zoom", 0.5);
        set(&programmers, session, wash.fixture_id, "focus", 0.5);
        let engine = Engine::new(programmers.clone());
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![mover.clone(), wash.clone(), media.clone()].into(),
                revision: 1,
                ..Default::default()
            })
            .unwrap();
        Self {
            engine,
            mover,
            wash,
            media,
        }
    }

    pub(super) fn frame(
        &self,
        options: RenderOptions,
    ) -> (PreparedOutputFrame, PreparedStaticFamilyFrame) {
        let capture = self.engine.prepare_output_frame(options);
        let frame = self.engine.prepare_static_family_frame(&capture, &[]);
        (capture, frame)
    }
}

pub(super) fn instances(fixture: &PatchedFixture) -> Vec<Uuid> {
    std::iter::once(fixture.fixture_id.0)
        .chain(fixture.multipatch.iter().map(|copy| copy.id))
        .collect()
}

/// Complete per-instance writes of `owner` on `target`: `(channel index, raw)` per instance.
pub(super) fn family_writes(
    fixture: &PatchedFixture,
    target: FixtureId,
    owner: ProgrammingOwner,
    controls: &[(usize, u32)],
) -> Vec<FamilyNativeWrite> {
    let mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
    instances(fixture)
        .into_iter()
        .flat_map(|instance_id| {
            controls.iter().map(move |&(index, raw)| {
                let channel = &mode.channels[index];
                FamilyNativeWrite {
                    owner,
                    target,
                    instance_id,
                    channel_index: index as u32,
                    channel_id: channel.id,
                    function_id: Some(channel.functions[0].id),
                    split: channel.split,
                    raw,
                }
            })
        })
        .collect()
}

const PAN: u32 = 0x1234;
const TILT: u32 = 0xabcd;
const COLOR: [(usize, u32); 3] = [(RED, 200), (RED + 1, 100), (RED + 2, 50)];
const MEDIA: [(usize, u32); 4] = [(0, 10), (1, 20), (2, 30), (3, 40)];

impl Rig {
    fn position(&self) -> Vec<FamilyNativeWrite> {
        family_writes(
            &self.mover,
            self.mover.fixture_id,
            ProgrammingOwner::Position,
            &[(0, PAN), (1, TILT)],
        )
    }
    fn color(&self) -> Vec<FamilyNativeWrite> {
        family_writes(
            &self.wash,
            self.wash.fixture_id,
            ProgrammingOwner::Color,
            &COLOR,
        )
    }
    fn optics(&self) -> Vec<FamilyNativeWrite> {
        let mut writes = family_writes(
            &self.wash,
            self.wash.fixture_id,
            ProgrammingOwner::Zoom,
            &[(ZOOM, 77)],
        );
        writes.extend(family_writes(
            &self.wash,
            self.wash.fixture_id,
            ProgrammingOwner::Focus,
            &[(FOCUS, 33)],
        ));
        writes
    }
    fn media(&self) -> Vec<FamilyNativeWrite> {
        family_writes(
            &self.media,
            self.media.fixture_id,
            ProgrammingOwner::Color,
            &MEDIA,
        )
    }
    fn all(&self) -> Vec<FamilyNativeWrite> {
        [self.position(), self.color(), self.optics(), self.media()].concat()
    }
}

pub(super) fn native_raw(output: &RenderResult, instance: Uuid) -> Vec<u32> {
    let rows: Vec<_> = output
        .physical
        .instances
        .iter()
        .filter(|row| row.instance_id == instance)
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].complete);
    rows[0].native_raw.to_vec()
}

#[test]
fn family_native_bytes_equal_the_fitted_writes_on_every_root_and_copy() {
    let rig = Rig::new(false);
    let baseline = rig.engine.render(Default::default()).unwrap();
    let wash_root = native_raw(&baseline, rig.wash.fixture_id.0);
    let (capture, mut frame) = rig.frame(Default::default());
    let token = capture.frame_token();
    let writes = rig.all();
    frame
        .project_family_native(&capture, &token, &writes)
        .unwrap();
    assert!(
        frame
            .project_family_native(&capture, &token, &rig.position())
            .is_err(),
        "a frame installs one native collection"
    );
    frame.project_family_native(&capture, &token, &[]).unwrap();
    let output = rig
        .engine
        .render_static_family_frame(&capture, frame)
        .unwrap();
    for write in &writes {
        assert_eq!(
            native_raw(&output, write.instance_id)[write.channel_index as usize],
            write.raw,
            "{write:?}"
        );
    }
    // Controls outside every footprint keep the ordinary scalar result.
    assert_eq!(native_raw(&output, rig.wash.fixture_id.0)[0], wash_root[0]);
    let universe = &output.universes[&1];
    for start in [0, 9] {
        assert_eq!(&universe[start..start + 4], &[0x12, 0x34, 0xab, 0xcd]);
    }
    for start in [19, 29] {
        assert_eq!(&universe[start + 1..start + 6], &[200, 100, 50, 77, 33]);
    }
    assert_eq!(&universe[39..43], &[10, 20, 30, 40]);
}

#[test]
fn a_write_outside_the_footprint_or_an_incomplete_owner_rejects_the_whole_frame() {
    let rig = Rig::new(false);
    let (capture, mut frame) = rig.frame(Default::default());
    let token = capture.frame_token();
    let mode = &rig.wash.definition.profile_snapshot.as_ref().unwrap().modes[0];
    let moved = |writes: &mut Vec<FamilyNativeWrite>, owner, from: usize, to: usize| {
        for write in writes
            .iter_mut()
            .filter(|w| w.owner == owner && w.channel_index == from as u32)
        {
            write.channel_index = to as u32;
            write.channel_id = mode.channels[to].id;
            write.function_id = Some(mode.channels[to].functions[0].id);
        }
    };
    // Complete in count, but Red's writes land on intensity, which no family footprint owns.
    let mut outside = rig.all();
    moved(&mut outside, ProgrammingOwner::Color, RED, 0);
    assert!(
        frame
            .project_family_native(&capture, &token, &outside)
            .is_err()
    );
    // Zoom writes Focus's control (Focus itself absent): outside the Zoom footprint.
    let mut foreign = rig.all();
    foreign.retain(|w| w.owner != ProgrammingOwner::Focus);
    moved(&mut foreign, ProgrammingOwner::Zoom, ZOOM, FOCUS);
    assert!(
        frame
            .project_family_native(&capture, &token, &foreign)
            .is_err()
    );
    let mut root_only = rig.all();
    let copy = rig.wash.multipatch[0].id;
    root_only.retain(|w| !(w.owner == ProgrammingOwner::Color && w.instance_id == copy));
    assert!(
        frame
            .project_family_native(&capture, &token, &root_only)
            .is_err(),
        "Color must write every physical copy"
    );
    let mut partial = rig.all();
    partial.retain(|w| !(w.owner == ProgrammingOwner::Color && w.channel_index == RED as u32));
    assert!(
        frame
            .project_family_native(&capture, &token, &partial)
            .is_err()
    );
    let mut wrong_family = rig.media();
    for write in &mut wrong_family {
        write.owner = ProgrammingOwner::Zoom;
    }
    assert!(
        frame
            .project_family_native(&capture, &token, &wrong_family)
            .is_err()
    );
    // Nothing was installed by any rejection: the valid collection is still the first one.
    frame
        .project_family_native(&capture, &token, &rig.all())
        .unwrap();
    let foreign_capture = rig.engine.prepare_output_frame(Default::default());
    let mut other = rig
        .engine
        .prepare_static_family_frame(&foreign_capture, &[]);
    assert!(
        other
            .project_family_native(&capture, &token, &rig.all())
            .is_err(),
        "another capture's token"
    );
}

#[test]
fn two_families_claiming_one_native_control_reject_even_when_they_agree() {
    let rig = Rig::new(true);
    let color = family_writes(
        &rig.wash,
        rig.wash.fixture_id,
        ProgrammingOwner::Color,
        &[COLOR[0], COLOR[1], COLOR[2], (ZOOM, 77)],
    );
    let zoom = family_writes(
        &rig.wash,
        rig.wash.fixture_id,
        ProgrammingOwner::Zoom,
        &[(ZOOM, 77)],
    );
    let (capture, mut frame) = rig.frame(Default::default());
    let token = capture.frame_token();
    let both = [color.clone(), zoom.clone()].concat();
    assert!(
        frame
            .project_family_native(&capture, &token, &both)
            .is_err()
    );
    let reversed = [zoom.clone(), color.clone()].concat();
    assert!(
        frame
            .project_family_native(&capture, &token, &reversed)
            .is_err()
    );
    // Each family alone is a complete, valid installation of the same control.
    frame
        .project_family_native(&capture, &token, &color)
        .unwrap();
    let (capture, mut frame) = rig.frame(Default::default());
    frame
        .project_family_native(&capture, &capture.frame_token(), &zoom)
        .unwrap();
    let output = rig
        .engine
        .render_static_family_frame(&capture, frame)
        .unwrap();
    assert_eq!(output.universes[&1][19 + ZOOM], 77);
}

#[test]
fn masters_and_blackout_leave_native_colour_writes_and_highlight_replaces_them() {
    let rig = Rig::new(false);
    let render = |options: RenderOptions| {
        let (capture, mut frame) = rig.frame(options);
        frame
            .project_family_native(&capture, &capture.frame_token(), &rig.all())
            .unwrap();
        rig.engine
            .render_static_family_frame(&capture, frame)
            .unwrap()
    };
    let wash =
        |output: &RenderResult, start: usize| output.universes[&1][start..start + 6].to_vec();
    let full = render(Default::default());
    let half = render(RenderOptions {
        grand_master: 0.5,
        ..Default::default()
    });
    // Masters scale level parameters only (2026-10-05). Colour, Zoom and Focus are not levels
    // and do not follow the virtual intensity here, so the native writes reach DMX unchanged.
    for start in [19, 29] {
        assert_eq!(wash(&full, start)[1..], [200, 100, 50, 77, 33]);
        assert_eq!(wash(&half, start)[1..], [200, 100, 50, 77, 33]);
    }
    assert_eq!(&half.universes[&1][0..4], &[0x12, 0x34, 0xab, 0xcd]);
    assert_eq!(native_raw(&half, rig.wash.fixture_id.0)[RED], 200);
    let dark = render(RenderOptions {
        blackout: true,
        ..Default::default()
    });
    for start in [19, 29] {
        assert_eq!(wash(&dark, start)[0], 0, "Blackout zeroes the level");
        assert_eq!(wash(&dark, start)[1..], [200, 100, 50, 77, 33]);
    }
    rig.engine.set_highlighted_fixtures([rig.wash.fixture_id]);
    let highlighted = render(Default::default());
    for start in [19, 29] {
        assert_eq!(
            wash(&highlighted, start),
            [255; 6],
            "Highlight replaces native writes"
        );
    }
    assert_eq!(&highlighted.universes[&1][39..43], &[10, 20, 30, 40]);
    rig.engine.clear_highlighted_fixtures();
    assert_eq!(
        wash(&render(Default::default()), 19)[1..],
        [200, 100, 50, 77, 33]
    );
}

/// TL-639 round 2: an owner's writes are validated once per consecutive run. Interleaving
/// owners must not change what is installed, what counts as complete or what is a duplicate.
#[test]
fn interleaved_owner_runs_install_exactly_like_grouped_writes() {
    let rig = Rig::new(false);
    let grouped = rig.all();
    // Alternate writes of every owner and instance so no run is longer than one write.
    let mut runs = grouped.clone();
    let (first, second): (Vec<_>, Vec<_>) =
        runs.drain(..).enumerate().partition(|(i, _)| i % 2 == 0);
    let interleaved = first
        .into_iter()
        .map(|(_, write)| write)
        .rev()
        .chain(second.into_iter().map(|(_, write)| write))
        .collect::<Vec<_>>();
    let render = |writes: &[FamilyNativeWrite]| {
        let (capture, mut frame) = rig.frame(Default::default());
        frame
            .project_family_native(&capture, &capture.frame_token(), writes)
            .map(|()| {
                rig.engine
                    .render_static_family_frame(&capture, frame)
                    .unwrap()
                    .universes[&1]
                    .to_vec()
            })
    };
    assert_eq!(render(&interleaved).unwrap(), render(&grouped).unwrap());
    // A duplicate in a later, separate run of the same owner is still a duplicate.
    let mut duplicated = interleaved.clone();
    duplicated.push(interleaved[0]);
    assert!(render(&duplicated).is_err());
    // An owner whose writes are spread over runs is complete only with all of them.
    let mut incomplete = interleaved;
    let missing = incomplete
        .iter()
        .position(|write| write.owner == ProgrammingOwner::Color)
        .unwrap();
    incomplete.remove(missing);
    assert!(render(&incomplete).is_err());
}

/// TL-639 round 4: a lane's kept installation of unchanged writes renders the full installation's
/// bytes, and any changed write installs anew.
#[test]
fn a_kept_native_installation_is_the_full_installation() {
    let rig = Rig::new(false);
    let writes = rig.all();
    let mut memo = crate::FamilyNativeMemo::default();
    let mut install = |writes: &[FamilyNativeWrite], kept: bool| {
        let (capture, mut frame) = rig.frame(Default::default());
        let token = capture.frame_token();
        if kept {
            let mut list = writes.to_vec();
            frame
                .project_family_native_kept(&capture, &token, &mut list, &mut memo)
                .unwrap();
        } else {
            frame
                .project_family_native(&capture, &token, writes)
                .unwrap();
        }
        let output = rig
            .engine
            .render_static_family_frame(&capture, frame)
            .unwrap();
        (output.universes.clone(), memo.kept_installations())
    };
    let (full, _) = install(&writes, false);
    let (first, kept) = install(&writes, true);
    assert_eq!(
        (first == full, kept),
        (true, 0),
        "the first installation is full"
    );
    let (again, kept) = install(&writes, true);
    assert_eq!(
        (again == full, kept),
        (true, 1),
        "unchanged writes are kept"
    );
    let mut changed = writes.clone();
    changed[0].raw = u32::from(changed[0].raw == 0);
    let (fresh_full, _) = install(&changed, false);
    let (fresh_kept, kept) = install(&changed, true);
    assert_eq!(
        (fresh_kept == fresh_full, kept),
        (true, 1),
        "a change installs anew"
    );
    assert_ne!(fresh_kept, full);
}
