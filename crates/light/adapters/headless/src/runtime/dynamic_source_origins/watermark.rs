use super::*;

impl DynamicSourceOrigins {
    /// Cold restore boundary only. Include unbound immutable history: the Playback which
    /// authored it may have been released while a Dynamic still retains the sampled source.
    /// Authored Cue transition ordinals are transport IDs, not Playback source occurrences.
    pub fn playback_source_occurrence_watermark(&self) -> u64 {
        self.records
            .values()
            .filter_map(|record| match &record.origin {
                DynamicSourceOrigin::StaticBaseline { sources } => Some(sources),
                _ => None,
            })
            .flatten()
            .filter(|source| matches!(source.source, DynamicStaticSource::Playback { .. }))
            .filter_map(|source| source.transition_ordinal)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests;
