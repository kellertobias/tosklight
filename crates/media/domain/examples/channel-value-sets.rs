//! Prints the canonical channel table's value sets as JSON.
//!
//! `channels.rs` says it is the single source the receivers, the API, UI metadata, the tests and
//! the GDTF export read. The shipped `.toskfixture` package is the one place that restated it, so
//! this dumps the sets in a form the package can be checked and repaired against, projected from
//! the same decoders that consume DMX rather than transcribed by hand.

fn main() {
    let mut blocks = Vec::new();
    for (block, channels) in [
        ("layer", media_domain::personality::LAYER_CHANNELS),
        ("master", media_domain::personality::MASTER_CHANNELS),
    ] {
        for spec in channels {
            let sets = spec.values.sets();
            if sets.is_empty() {
                continue;
            }
            let rendered = sets
                .iter()
                .map(|set| {
                    format!(
                        r#"{{"name":{:?},"from":{},"to":{},"step":{},"implemented":{}}}"#,
                        set.name, set.from, set.to, set.step, set.implemented
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            blocks.push(format!(
                r#"{{"block":"{block}","offset":{},"name":{:?},"default":{},"sets":[{rendered}]}}"#,
                spec.offset, spec.name, spec.default_value
            ));
        }
    }
    println!("[{}]", blocks.join(","));
}
