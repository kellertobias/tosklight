//! Bounded, atomically replaced occurrence outcomes. A consumed start is durable before playback.

use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const MAX_RECORDS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    key: String,
    outcome: String,
    at_millis: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    records: Vec<Record>,
    replay_floor_millis: Option<i64>,
}

pub(super) struct Journal {
    path: Option<PathBuf>,
    stored: Stored,
    pub consumed: HashSet<String>,
    pub error: Option<String>,
}

impl Journal {
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut error = None;
        let stored = path.as_ref().map_or_else(Stored::default, |path| {
            match std::fs::read(path) {
                Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|cause| {
                    error = Some(format!("Cannot read occurrence journal {}: {cause}. The original file is preserved; single-pass triggers are suspended.", path.display()));
                    Stored::default()
                }),
                Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => Stored::default(),
                Err(cause) => {
                    error = Some(format!("Cannot read occurrence journal {}: {cause}. Single-pass triggers are suspended.", path.display()));
                    Stored::default()
                }
            }
        });
        let consumed = stored
            .records
            .iter()
            .map(|record| record.key.clone())
            .collect();
        Self {
            path,
            stored,
            consumed,
            error,
        }
    }

    pub fn floor(&self) -> Option<i64> {
        self.stored.replay_floor_millis
    }

    pub fn latest_for_events(&self, event_ids: &HashSet<String>) -> Option<(&str, &str)> {
        if event_ids.is_empty() {
            return None;
        }
        self.stored.records.iter().rev().find_map(|record| {
            let event_id = record.key.split_once(':')?.0;
            event_ids
                .contains(event_id)
                .then_some((event_id, record.outcome.as_str()))
        })
    }

    pub fn consume(&mut self, values: impl IntoIterator<Item = (String, String, i64)>) -> bool {
        if self.error.is_some() {
            return false;
        }
        let mut changed = false;
        for (key, outcome, at_millis) in values {
            if self.consumed.insert(key.clone()) {
                self.stored.records.push(Record {
                    key,
                    outcome,
                    at_millis,
                });
                changed = true;
            } else if let Some(record) = self
                .stored
                .records
                .iter_mut()
                .find(|record| record.key == key)
            {
                if record.outcome != outcome
                    && matches!(outcome.as_str(), "completed" | "interrupted" | "failed")
                {
                    record.outcome = outcome;
                    changed = true;
                }
            }
        }
        if !changed {
            return true;
        }
        if self.stored.records.len() > MAX_RECORDS {
            let remove = self.stored.records.len() - MAX_RECORDS;
            for record in self.stored.records.drain(..remove) {
                self.consumed.remove(&record.key);
                self.stored.replay_floor_millis = Some(
                    self.stored
                        .replay_floor_millis
                        .map_or(record.at_millis, |floor| floor.max(record.at_millis)),
                );
            }
        }
        if let Err(cause) = self.save() {
            self.error = Some(format!(
                "Cannot save occurrence outcomes: {cause}. Single-pass triggers are suspended."
            ));
            return false;
        }
        true
    }

    fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.pending");
        let bytes = serde_json::to_vec(&self.stored)?;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_outcomes_survive_duplicate_crossings_and_serialization() {
        let mut journal = Journal::new(None);
        let key = "event:2026-11-22T10:00".to_owned();
        assert!(journal.consume([(key.clone(), "started".into(), 100)]));
        assert!(journal.consume([(key.clone(), "completed".into(), 100)]));
        assert!(journal.consume([(key.clone(), "suppressed".into(), 100)]));
        assert!(journal.consume([(key.clone(), "started".into(), 100)]));
        let stored: Stored =
            serde_json::from_slice(&serde_json::to_vec(&journal.stored).unwrap()).unwrap();
        assert_eq!(stored.records.len(), 1);
        assert_eq!(stored.records[0].outcome, "completed");
        assert!(journal.consumed.contains(&key));
        assert_eq!(
            journal.latest_for_events(&HashSet::from(["event".into()])),
            Some(("event", "completed"))
        );
    }

    #[test]
    fn bounded_retention_keeps_a_replay_floor_for_evicted_occurrences() {
        let mut journal = Journal::new(None);
        assert!(journal.consume((0..=MAX_RECORDS).map(|index| (
            format!("event:{index}"),
            "suppressed".into(),
            index as i64
        ))));
        assert_eq!(journal.stored.records.len(), MAX_RECORDS);
        assert_eq!(journal.consumed.len(), MAX_RECORDS);
        assert_eq!(journal.floor(), Some(0));
        assert!(!journal.consumed.contains("event:0"));
        assert!(journal.consumed.contains(&format!("event:{MAX_RECORDS}")));
    }
}
