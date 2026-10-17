//! Canonical full-frame comparison with bounded per-frame retention; all detail is hashed.
use super::*;
#[derive(PartialEq)]
pub(super) struct Observed {
    pub(super) common: Value,
    pub(super) metadata: Value,
}
pub(super) fn observe(
    bench: &LiveOutputBench,
    frame: &light_engine::RenderResult,
    hybrid: bool,
) -> Observed {
    let universes = frame
        .universes
        .iter()
        .map(|(u, b)| (*u, b.to_vec()))
        .collect::<BTreeMap<_, _>>();
    let patched = frame
        .patched_slots
        .iter()
        .map(|(u, n)| (*u, *n))
        .collect::<BTreeMap<_, _>>();
    let mut values = frame
        .resolved_values
        .values()
        .iter()
        .map(|((id, key), value)| json!([id.0, key, serde_json::to_value(value).unwrap()]))
        .collect::<Vec<_>>();
    values.sort_by_cached_key(|v| serde_json::to_vec(&canonical(v.clone())).unwrap());
    let mut heads = frame
        .profile_visualization_values
        .iter()
        .map(|((id, key), value)| json!([id.0, key, serde_json::to_value(value).unwrap()]))
        .collect::<Vec<_>>();
    heads.sort_by_cached_key(|v| serde_json::to_vec(&canonical(v.clone())).unwrap());
    let mut physical = frame
        .physical
        .instances
        .iter()
        .map(|p| {
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                p.fixture_id,
                p.instance_id,
                p.native_raw,
                p.colors(),
                p.axes(),
                p.lenses(),
                p.optics()
            )
        })
        .collect::<Vec<_>>();
    physical.sort();
    let mut diagnostics = frame
        .physical
        .instances
        .iter()
        .map(|p| {
            format!(
                "{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}/{:?}",
                p.fixture_id,
                p.instance_id,
                p.native_identity,
                p.complete,
                p.color_support,
                p.position_support,
                p.optics_support,
                (p.color_diagnostic.as_ref(), p.position_diagnostic.as_ref()),
                p.optics_diagnostic
            )
        })
        .collect::<Vec<_>>();
    diagnostics.sort();
    Observed {
        common: canonical(
            json!({"universes":universes,"patched":patched,"values":values,"heads":heads,"physical":physical,"points":sorted_debug(frame.points.iter()),"mounts":sorted_debug(frame.mounts.mounts().iter()),"routes":serde_json::to_value(frame.routes.as_ref()).unwrap()}),
        ),
        metadata: canonical(
            json!({"generation":frame.generation,"revision":frame.revision,"sampled_at":frame.sampled_at,"diagnostics":diagnostics,"color":bench.readout_digest(frame),"hybrid":hybrid,"requirements":hybrid.then(||bench.test_take_requirement_rows())}),
        ),
    }
}

fn sorted_debug<T: std::fmt::Debug>(values: impl Iterator<Item = T>) -> Vec<String> {
    let mut rows = values.map(|v| format!("{v:?}")).collect::<Vec<_>>();
    rows.sort();
    rows
}
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(rows) => serde_json::to_value(
            rows.into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect::<BTreeMap<_, _>>(),
        )
        .unwrap(),
        Value::Array(rows) => Value::Array(rows.into_iter().map(canonical).collect()),
        value => value,
    }
}
pub(super) fn evidence(tick: usize, observed: &[Observed], witness: Option<&Path>) -> Value {
    let common = observed
        .iter()
        .map(|v| serde_json::to_vec(&v.common).unwrap())
        .collect::<Vec<_>>();
    if common[0] != common[1] {
        if let Some(path) = witness {
            write_private(
                path,
                &json!({"tick":tick,"arms":observed.iter().map(|v|json!({"common":v.common,"metadata":v.metadata})).collect::<Vec<_>>()}),
            );
        }
        panic!("legacy_pair_common_output_mismatch");
    }
    json!({"tick":tick,"common_sha256":sha(&common[0]),"dmx_sha256":sha(&serde_json::to_vec(&observed[0].common["universes"]).unwrap()),"full_arm_metadata_sha256":observed.iter().map(|v|sha(&serde_json::to_vec(&v.metadata).unwrap())).collect::<Vec<_>>()})
}
pub(super) fn write_private(path: &Path, value: &Value) {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .unwrap()
        .write_all(&serde_json::to_vec(value).unwrap())
        .unwrap();
}
