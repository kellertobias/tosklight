use super::*;
use light_core::FixtureId;
use light_fixture::{PatchedFixture, SplitPatch};
use std::collections::BTreeMap;

type Slots = HashMap<(u16, u16), (String, String)>;

pub(super) fn findings(
    document: &PortableShowDocument,
    runtime: &EngineSnapshot,
) -> Vec<ShowHealthFinding> {
    let records = document
        .objects_of_kind("patched_fixture")
        .filter_map(|object| {
            let patch = light_fixture::PortablePatchedFixtureRecord::decode(object.body().clone())
                .ok()?
                .patch()
                .ok()?;
            Some((patch.fixture_id, object))
        })
        .collect::<HashMap<_, _>>();
    let mut scanner = Scanner {
        result: Vec::new(),
        occupied: HashMap::new(),
        overlaps: HashSet::new(),
        layout_placements: authored_layout_placements(document, "positions3d", &["x", "y", "z"]),
        legacy_layout_placements: authored_layout_placements(document, "positions", &["x", "y"]),
        routed: runtime
            .routes
            .iter()
            .filter(|route| route.enabled)
            .map(|route| route.logical_universe)
            .collect(),
    };
    for fixture in runtime.fixtures.iter() {
        if let Some(object) = records.get(&fixture.fixture_id) {
            scanner.fixture(object, fixture);
        }
    }
    scanner.result
}

struct Scanner {
    result: Vec<ShowHealthFinding>,
    occupied: Slots,
    overlaps: HashSet<(String, String)>,
    routed: HashSet<u16>,
    layout_placements: HashSet<Uuid>,
    legacy_layout_placements: HashSet<Uuid>,
}

impl Scanner {
    fn fixture(&mut self, object: &PortableShowObject, fixture: &PatchedFixture) {
        let label = format!(
            "Fixture {} {}",
            fixture
                .fixture_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| fixture.fixture_id.0.to_string()),
            fixture.name
        );
        if fixture.definition.patch_policy() != light_fixture::PatchPolicy::Internal {
            self.position(object, fixture, &label);
        }
        if matches!(
            fixture.definition.patch_policy(),
            light_fixture::PatchPolicy::VisualOnly | light_fixture::PatchPolicy::Internal
        ) {
            return;
        }
        let footprints = fixture.definition.split_footprints();
        self.instance(
            object,
            fixture.fixture_id,
            &label,
            None,
            &normalized(
                &fixture.split_patches,
                fixture.universe,
                fixture.address,
                &footprints,
            ),
            &footprints,
        );
        for instance in &fixture.multipatch {
            self.instance(
                object,
                fixture.fixture_id,
                &label,
                Some(instance.id),
                &normalized(
                    &instance.split_patches,
                    instance.universe,
                    instance.address,
                    &footprints,
                ),
                &footprints,
            );
        }
    }

    fn position(&mut self, object: &PortableShowObject, fixture: &PatchedFixture, label: &str) {
        let body = object.body();
        let root_placed = self.layout_placements.contains(&fixture.fixture_id.0)
            || self
                .legacy_layout_placements
                .contains(&fixture.fixture_id.0);
        if !root_placed
            && body.get("location").is_none_or(|location| {
                !["x", "y", "z"]
                    .iter()
                    .all(|axis| location[*axis].is_number())
            })
        {
            self.result.push(finding(
                object,
                "position-missing",
                ShowHealthSeverity::Warning,
                format!("{label} has no valid stored Stage position."),
                Some(fixture.fixture_id.0),
                false,
            ));
        } else if !root_placed && !fixture.stage_position_set && fixture.position_master.is_none() {
            self.result.push(finding(
                object,
                "position-unplaced",
                ShowHealthSeverity::Warning,
                format!("{label} has no authored Stage placement. Set its location to confirm placement, including the origin."),
                Some(fixture.fixture_id.0),
                false,
            ));
        }
        for instance in &fixture.multipatch {
            if !instance.stage_position_set && !self.layout_placements.contains(&instance.id) {
                let mut row = finding(
                    object,
                    &format!("position-unplaced:{}", instance.id),
                    ShowHealthSeverity::Warning,
                    format!(
                        "{label} copy {} has no authored Stage placement.",
                        instance.id
                    ),
                    Some(fixture.fixture_id.0),
                    false,
                );
                row.multipatch_instance_id = Some(instance.id);
                self.result.push(row);
            }
        }
    }

    fn instance(
        &mut self,
        object: &PortableShowObject,
        owner: FixtureId,
        label: &str,
        instance: Option<Uuid>,
        patches: &[SplitPatch],
        footprints: &BTreeMap<u16, u16>,
    ) {
        let instance_label = instance.map_or_else(|| "primary".into(), |id| id.to_string());
        let mut push = |key: String, severity, message| {
            let mut row = finding(object, &key, severity, message, Some(owner.0), false);
            row.multipatch_instance_id = instance;
            self.result.push(row);
        };
        let mut assigned = HashSet::new();
        for patch in patches {
            let key = format!("{instance_label}:{}", patch.split);
            if !assigned.insert(patch.split) {
                push(
                    format!("duplicate:{key}"),
                    ShowHealthSeverity::Error,
                    format!("{label} repeats split {} in {instance_label}.", patch.split),
                );
            }
            let Some(footprint) = footprints.get(&patch.split) else {
                push(
                    format!("split:{key}"),
                    ShowHealthSeverity::Error,
                    format!("{label} has an unknown patch split {}.", patch.split),
                );
                continue;
            };
            let (Some(universe), Some(address)) = (patch.universe, patch.address) else {
                let (severity, message) = if patch.universe.is_some() || patch.address.is_some() {
                    (
                        ShowHealthSeverity::Error,
                        format!(
                            "{label} · {instance_label} split {} needs both universe and address, or neither.",
                            patch.split
                        ),
                    )
                } else {
                    (
                        ShowHealthSeverity::Info,
                        format!(
                            "{label} · {instance_label} split {} is unpatched; it remains programmable in the show.",
                            patch.split
                        ),
                    )
                };
                push(format!("unpatched:{key}"), severity, message);
                continue;
            };
            let end = u32::from(address) + u32::from(*footprint).saturating_sub(1);
            if address == 0 || end > 512 {
                push(
                    format!("bounds:{key}"),
                    ShowHealthSeverity::Error,
                    format!(
                        "{label} patch {universe}.{address} extends outside DMX slots 1 to 512."
                    ),
                );
            }
            if !self.routed.contains(&universe) {
                push(
                    format!("route:{key}"),
                    ShowHealthSeverity::Warning,
                    format!("{label} uses universe {universe}, which has no enabled output route."),
                );
            }
            let identity = format!("{}:{key}", object.key().id());
            for channel in address.max(1)..=end.min(512) as u16 {
                if let Some((previous, previous_label)) =
                    self.occupied.get(&(universe, channel)).cloned()
                {
                    if self.overlaps.insert((previous.clone(), identity.clone())) {
                        push(
                            format!("overlap:{previous}:{key}"),
                            ShowHealthSeverity::Warning,
                            format!(
                                "DMX overlap on universe {universe}, slot {channel}: {label} ({instance_label}) and {previous_label}."
                            ),
                        );
                    }
                } else {
                    self.occupied.insert(
                        (universe, channel),
                        (identity.clone(), format!("{label} ({instance_label})")),
                    );
                }
            }
        }
        for split in footprints.keys().filter(|split| !assigned.contains(split)) {
            push(
                format!("missing-split:{instance_label}:{split}"),
                ShowHealthSeverity::Error,
                format!(
                    "{label} has no assignment for expected split {split} in {instance_label}."
                ),
            );
        }
    }
}

fn normalized(
    patches: &[SplitPatch],
    universe: Option<u16>,
    address: Option<u16>,
    footprints: &BTreeMap<u16, u16>,
) -> Vec<SplitPatch> {
    if !patches.is_empty() {
        return patches.to_vec();
    }
    let split = if footprints.len() == 1 {
        *footprints.keys().next().unwrap()
    } else {
        1
    };
    vec![SplitPatch {
        split,
        universe,
        address,
    }]
}

fn authored_layout_placements(
    document: &PortableShowDocument,
    field: &str,
    axes: &[&str],
) -> HashSet<Uuid> {
    document
        .object("stage_layout", "main")
        .and_then(|layout| layout.body().get(field))
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|entries| entries.iter())
        .filter_map(|(id, value)| {
            axes.iter()
                .all(|axis| value[*axis].as_f64().is_some_and(f64::is_finite))
                .then(|| Uuid::parse_str(id).ok())
                .flatten()
        })
        .collect()
}
