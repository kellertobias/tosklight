use super::*;

pub(super) fn load_engine_snapshot(entry: &ShowEntry) -> Result<EngineSnapshot, String> {
    if entry.name == default_show::name() {
        default_show::upgrade(&entry.path).map_err(|error| error.to_string())?;
    }
    reconcile_show_schema_defaults(entry)?;
    reconcile_show_logical_heads(entry)?;
    reconcile_show_cue_identities(entry)?;
    load_engine_snapshot_with_override(entry, None)
}
