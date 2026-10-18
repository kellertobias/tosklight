//! Exercise logical chunk preparation/replay, not operating-system thread scheduling.
use super::*;
use light_core::programming::{
    ProgrammingFieldScope, ProgrammingFieldTransfer, ProgrammingTraceField,
};
use std::cell::RefCell;

type Read = (FixtureId, ProgrammingComponent);
#[derive(Default)]
struct Fork {
    pending: RefCell<Vec<Read>>,
    committed: RefCell<Vec<Read>>,
    reads: Cell<usize>,
    closes: Cell<usize>,
    missed: Cell<bool>,
    missing: Option<FixtureId>,
}
impl DynamicValueSourceResolver for Fork {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        self.pending
            .borrow_mut()
            .push((target, address.component.expect("Angle Current")));
        if self.missing == Some(target) {
            self.missed.set(true);
            return None;
        }
        match address.component {
            Some(ProgrammingComponent::Pan) => Some(DynamicValue::Scalar(5.0)),
            Some(ProgrammingComponent::Tilt) => Some(DynamicValue::Scalar(6.0)),
            _ => None,
        }
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        panic!("not a preset fixture")
    }
}
impl PreparationSources for Fork {
    // No opt-in: existing custom implementations retain every original close.
    fn finish_controller(&self) -> Option<(usize, usize)> {
        self.closes.set(self.closes.get() + 1);
        if self.missed.get() {
            self.pending.borrow_mut().clear();
            return None;
        }
        self.committed
            .borrow_mut()
            .extend(self.pending.borrow_mut().drain(..));
        Some((self.committed.borrow().len(), 0))
    }
}
struct Allowed<'a>(&'a Fork);
impl DynamicValueSourceResolver for Allowed<'_> {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.0.current(target, address)
    }
    fn preset(
        &self,
        source: &DynamicPresetSourceBinding,
        instance: Uuid,
        target: FixtureId,
    ) -> Option<DynamicValue> {
        self.0.preset(source, instance, target)
    }
}
impl PreparationSources for Allowed<'_> {
    fn source_free_preparation(&self) -> bool {
        !self.0.missed.get()
    }
    fn finish_controller(&self) -> Option<(usize, usize)> {
        self.0.finish_controller()
    }
}
struct Workers {
    chunks: usize,
    allowed: bool,
    missing: Option<FixtureId>,
    initially_missed: bool,
    logs: Vec<Vec<Read>>,
    appended: Vec<Read>,
    ranges: Vec<(usize, (usize, usize), (usize, usize))>,
    closes: usize,
    reads: usize,
}
impl Workers {
    fn new(chunks: usize, allowed: bool) -> Self {
        Self {
            chunks,
            allowed,
            missing: None,
            initially_missed: false,
            logs: Vec::new(),
            appended: Vec::new(),
            ranges: Vec::new(),
            closes: 0,
            reads: 0,
        }
    }
}
impl PreparationWorkers for Workers {
    fn chunks(&self) -> usize {
        self.chunks
    }
    fn run(
        &mut self,
        prepare: &(dyn Fn(usize, &dyn PreparationSources) -> PreparedChunk + Sync),
    ) -> Result<Vec<PreparedChunk>, TransitionError> {
        self.logs.clear();
        self.appended.clear();
        self.ranges.clear();
        self.closes = 0;
        self.reads = 0;
        let mut outputs = Vec::new();
        for chunk in 0..self.chunks {
            let fork = Fork {
                missing: self.missing,
                ..Default::default()
            };
            fork.missed.set(self.initially_missed);
            assert!(fork.committed.borrow().is_empty() && fork.pending.borrow().is_empty());
            let output = if self.allowed {
                prepare(chunk, &Allowed(&fork))
            } else {
                prepare(chunk, &fork)
            };
            self.closes += fork.closes.get();
            self.reads += fork.reads.get();
            self.logs.push(fork.committed.into_inner());
            outputs.push(output);
        }
        Ok(outputs)
    }
    fn run_completion(
        &mut self,
        _: &(dyn Fn(usize, &dyn PreparationSources) -> crate::CompletedChunk + Sync),
    ) -> Result<Vec<crate::CompletedChunk>, TransitionError> {
        panic!("not completion")
    }
    fn append_logs(&mut self, chunk: usize, from: (usize, usize), to: (usize, usize)) {
        assert_eq!((from.1, to.1), (0, 0));
        assert!(from.0 <= to.0 && to.0 <= self.logs[chunk].len());
        self.appended
            .extend_from_slice(&self.logs[chunk][from.0..to.0]);
        self.ranges.push((chunk, from, to));
    }
}
fn sample_at(controller: u128, target: u128, lane: u128, expression: E) -> DynamicRuntimeSample {
    let mut input = sample(lane, expression);
    input.controller_id = Uuid::from_u128(controller);
    input.target = FixtureId(Uuid::from_u128(target));
    if let E::Programming {
        address,
        occurrence,
        dependency_occurrence,
        ..
    } = &mut input.expression
    {
        *occurrence =
            Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(1000 + controller)).unwrap());
        *dependency_occurrence = Some(DynamicSourceDependency::compatible(*occurrence, address));
    }
    input
}
fn plain(controller: u128, target: u128, value: f32) -> Vec<DynamicRuntimeSample> {
    [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, component)| {
        sample_at(
            controller,
            target,
            10 + index as u128,
            color(component, value),
        )
    })
    .collect()
}
fn queried(controller: u128, target: u128) -> Vec<DynamicRuntimeSample> {
    vec![
        sample_at(controller, target, 20, current(ProgrammingComponent::Pan)),
        sample_at(
            controller,
            target,
            21,
            angle(ProgrammingComponent::Tilt, 8.0),
        ),
    ]
}
fn observed(prepared: PreparedDynamicFamilySamples<'_>) -> Vec<String> {
    let mut result = vec![format!("{:?}", prepared.requirements)];
    result.extend(
        prepared
            .legacy
            .iter()
            .map(|sample| format!("{:?}", sample.expression)),
    );
    for group in prepared.families {
        let base = match group.owner {
            ProgrammingOwner::Color => semantic(ColorIntent::default()),
            ProgrammingOwner::Position => angles(0.0, 0.0),
            _ => panic!("unexpected owner"),
        };
        let mut scratch = RetainedFamilyCompositionScratch::default();
        let value = compose_retained_dynamic_family_traced(
            group.owner,
            &base,
            &group.samples,
            &FamilyCompositionContext {
                edit: FamilyEditContext {
                    color_model: Some(&VirtualColorAuthoringV1),
                    ..Default::default()
                },
                ..Default::default()
            },
            &Frame,
            &mut scratch,
        )
        .unwrap();
        let fields = ProgrammingFieldScope::new([
            ProgrammingTraceField::ColorXyz,
            ProgrammingTraceField::ColorRecipeRed,
            ProgrammingTraceField::ColorRecipeGreen,
            ProgrammingTraceField::ColorRecipeBlue,
            ProgrammingTraceField::ColorRecipeAmber,
            ProgrammingTraceField::WhiteBlend,
            ProgrammingTraceField::Uv,
            ProgrammingTraceField::Pan,
            ProgrammingTraceField::Tilt,
        ]);
        let trace = scratch.family_trace();
        let root = trace.root().expect("nonempty traced family");
        result.push(format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            group.target,
            group.owner,
            value,
            trace.query_fields_with_base(root, &fields),
            trace.control_sources_for_fields(root, &fields)
        ));
    }
    result
}
fn run(
    input: &[DynamicRuntimeSample],
    requirements: &[DynamicFamilyPreparationRequirement],
    scratch: &mut DynamicFamilyPreparationScratch,
    workers: &mut Workers,
) -> (Result<Vec<String>, String>, usize) {
    let frame = Sources {
        pan: Some(5.0),
        tilt: Some(6.0),
        ..Default::default()
    };
    let result = prepare_dynamic_family_samples_in_parallel(
        input,
        requirements,
        &frame,
        None,
        scratch,
        workers,
    )
    .map(observed)
    .map_err(|error| format!("{error:?}"));
    (result, frame.reads.get())
}

#[test]
fn successful_plain_controllers_skip_empty_closes_only_with_explicit_opt_in() {
    for chunks in [1, 2, 4] {
        let (mut fast, mut old) = (
            DynamicFamilyPreparationScratch::default(),
            DynamicFamilyPreparationScratch::default(),
        );
        let (mut yes, mut no) = (Workers::new(chunks, true), Workers::new(chunks, false));
        for value in [0.2, 0.7, 0.4] {
            let input = (1..=4)
                .flat_map(|controller| plain(controller, 3 + 16 * (controller - 1), value))
                .collect::<Vec<_>>();
            let requirement = DynamicFamilyPreparationRequirement {
                target: input[0].target,
                owner: ProgrammingOwner::Color,
                rank: rank(&input[0]),
                reason: DynamicFamilyPreparationRequirementReason::Transition(
                    TransitionRequirement::CompatibleOwners,
                ),
            };
            let actual = run(&input, &[requirement.clone()], &mut fast, &mut yes);
            let expected = run(&input, &[requirement], &mut old, &mut no);
            assert!(actual.0.is_ok());
            assert_eq!(
                actual.0.as_ref().unwrap().len(),
                5,
                "four nonempty families plus requirement observation"
            );
            assert_eq!(actual, expected);
            assert_eq!((yes.closes, no.closes), (0, 4)); // Fail-first structural witness.
            assert_eq!((yes.reads, no.reads), (0, 0));
            assert_eq!(yes.ranges, no.ranges);
            assert_eq!(yes.ranges.len(), 4);
            assert_eq!(
                yes.ranges
                    .iter()
                    .map(|(chunk, _, _)| *chunk)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                chunks
            );
            assert!(
                yes.ranges
                    .iter()
                    .all(|(_, from, to)| *from == (0, 0) && *to == (0, 0))
            );
            assert!(fast.cache.is_empty() && old.cache.is_empty());
        }
    }
}

#[test]
fn plain_between_queried_controllers_keeps_initial_and_previous_log_boundaries() {
    for chunks in [1, 2, 4] {
        let input = [
            plain(1, 3, 0.2),
            queried(2, 3),
            plain(3, 3, 0.4),
            queried(4, 3),
            plain(5, 3, 0.8),
        ]
        .concat();
        let (mut fast, mut old) = (
            DynamicFamilyPreparationScratch::default(),
            DynamicFamilyPreparationScratch::default(),
        );
        let (mut yes, mut no) = (Workers::new(chunks, true), Workers::new(chunks, false));
        let actual = run(&input, &[], &mut fast, &mut yes);
        let expected = run(&input, &[], &mut old, &mut no);
        assert!(actual.0.is_ok());
        assert_eq!(actual, expected);
        assert_eq!((yes.closes, no.closes), (2, 5));
        assert_eq!((yes.reads, no.reads), (2, 2));
        assert_eq!(yes.appended, no.appended);
        assert_eq!(yes.appended.len(), 2);
        assert_eq!(yes.ranges, no.ranges);
        assert_eq!(
            yes.ranges
                .iter()
                .map(|(_, a, b)| (a.0, b.0))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 1), (1, 1), (1, 2), (2, 2)]
        );
    }
}

#[test]
fn missed_query_stops_chunk_and_replays_later_plain_controller_on_frame() {
    for chunks in [1, 2, 4] {
        // Shards 3 and 4 share a chunk for all three tested chunk counts.
        let input = [
            queried(1, 3),
            plain(2, 3, 0.4),
            queried(3, 4),
            plain(4, 4, 0.8),
        ]
        .concat();
        let (mut fast, mut old) = (
            DynamicFamilyPreparationScratch::default(),
            DynamicFamilyPreparationScratch::default(),
        );
        let (mut yes, mut no) = (Workers::new(chunks, true), Workers::new(chunks, false));
        yes.missing = Some(FixtureId(Uuid::from_u128(4)));
        no.missing = yes.missing;
        let actual = run(&input, &[], &mut fast, &mut yes);
        let expected = run(&input, &[], &mut old, &mut no);
        assert!(actual.0.is_ok());
        assert_eq!(actual, expected);
        assert!(actual.1 > 0);
        assert_eq!((yes.closes, no.closes), (2, 3));
        assert_eq!(yes.appended, no.appended);
        assert_eq!(yes.appended.len(), 1);
        assert_eq!(yes.ranges, no.ranges);
        assert!(
            yes.appended
                .iter()
                .all(|(target, _)| *target == FixtureId(Uuid::from_u128(3)))
        );
    }
}

#[test]
fn errors_always_close_before_handling_and_same_scratch_recovers() {
    for chunks in [1, 2, 4] {
        let (mut fast, mut old) = (
            DynamicFamilyPreparationScratch::default(),
            DynamicFamilyPreparationScratch::default(),
        );
        for fault in 0..4 {
            let mut input = [plain(1, 3, 0.2), plain(2, 3, 0.4)].concat();
            match fault {
                0 => input[3].activation_mix = f32::NAN,
                1 => {
                    if let E::Programming { value, .. } = &mut input[3].expression {
                        *value = DynamicValue::Scalar(f32::NAN);
                    }
                }
                2 => input[4].lane_id = input[3].lane_id,
                _ => {
                    if let E::Programming {
                        dependency_occurrence,
                        ..
                    } = &mut input[3].expression
                    {
                        *dependency_occurrence = Some(DynamicSourceDependency::mapped(
                            None,
                            ProgrammingFieldTransfer {
                                identity: ProgrammingFieldScope::from_component(
                                    ProgrammingComponent::Focus,
                                ),
                                remap: Arc::from([]),
                            },
                        ));
                    }
                }
            }
            let (mut yes, mut no) = (Workers::new(chunks, true), Workers::new(chunks, false));
            let actual = run(&input, &[], &mut fast, &mut yes);
            let expected = run(&input, &[], &mut old, &mut no);
            assert!(actual.0.is_err());
            assert_eq!(actual, expected);
            assert_eq!((yes.closes, no.closes), (1, 2));
            assert!(fast.families.is_empty() && old.families.is_empty());
            let valid = plain(1, 3, 0.6);
            assert_eq!(
                run(&valid, &[], &mut fast, &mut yes),
                run(&valid, &[], &mut old, &mut no)
            );
            assert_eq!((yes.closes, no.closes), (0, 1));
        }
        let mut input = plain(1, 3, 0.2);
        input[0].activation_mix = f32::NAN;
        let (mut yes, mut no) = (Workers::new(chunks, true), Workers::new(chunks, false));
        yes.initially_missed = true;
        no.initially_missed = true;
        let actual = run(&input, &[], &mut fast, &mut yes);
        assert!(actual.0.is_err());
        assert_eq!(actual, run(&input, &[], &mut old, &mut no));
        assert_eq!((yes.closes, no.closes), (1, 1));
        assert!(yes.ranges.is_empty() && no.ranges.is_empty());
    }
}
