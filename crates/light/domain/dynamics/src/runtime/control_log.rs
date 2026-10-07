//! Bounded, process-local control history. A cursor names a position between records;
//! it is not a timestamp and must not be persisted as a portable runtime checkpoint.

use std::{collections::VecDeque, num::NonZeroUsize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlCursor {
    epoch: Uuid,
    sequence: u64,
}

impl ControlCursor {
    pub(crate) fn distance_to(self, other: Self) -> Option<usize> {
        if self.epoch != other.epoch {
            return None;
        }
        usize::try_from(other.sequence.checked_sub(self.sequence)?).ok()
    }

    /// Move within this epoch; the batch owner validates the requested prefix length.
    pub(crate) fn advance(self, count: usize) -> Option<Self> {
        Some(Self {
            sequence: self.sequence.checked_add(u64::try_from(count).ok()?)?,
            ..self
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlLogError {
    WrongEpoch,
    HistoryLost,
    FutureCursor,
}

impl std::fmt::Display for ControlLogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::WrongEpoch => "control cursor belongs to another runtime history",
            Self::HistoryLost => "requested control history is no longer retained",
            Self::FutureCursor => "control cursor is ahead of the committed history",
        })
    }
}

impl std::error::Error for ControlLogError {}

#[derive(Clone, Debug)]
pub(crate) struct ControlBatch<T: Clone> {
    pub(crate) from: ControlCursor,
    pub(crate) to: ControlCursor,
    pub(crate) entries: Vec<T>,
}

/// The owner supplies committed records and acknowledges the oldest cursor still needed by
/// its consumers. Storage pressure never blocks a writer: lagging readers get `HistoryLost`.
/// Cloning creates independent mutable storage with the same cursor lineage.
#[derive(Clone, Debug)]
pub(crate) struct ControlLog<T: Clone> {
    capacity: NonZeroUsize,
    cursor: ControlCursor,
    entries: VecDeque<T>,
}

impl<T: Clone> ControlLog<T> {
    pub(crate) fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            cursor: ControlCursor {
                epoch: Uuid::new_v4(),
                sequence: 0,
            },
            entries: VecDeque::new(),
        }
    }

    pub(crate) fn cursor(&self) -> ControlCursor {
        self.cursor
    }

    pub(crate) fn append(&mut self, entries: Vec<T>) {
        let count = u64::try_from(entries.len()).expect("record count fits in u64");
        if self.cursor.sequence.checked_add(count).is_none() {
            self.reset();
        }
        self.cursor.sequence += count;
        let skip = entries.len().saturating_sub(self.capacity.get());
        if entries.len() >= self.capacity.get() {
            self.entries.clear();
        }
        for entry in entries.into_iter().skip(skip) {
            if self.entries.len() == self.capacity.get() {
                self.entries.pop_front();
            }
            self.entries.push_back(entry);
        }
    }

    pub(crate) fn read(&self, cursor: ControlCursor) -> Result<ControlBatch<T>, ControlLogError> {
        let offset = self.offset(cursor)?;
        Ok(ControlBatch {
            from: cursor,
            to: self.cursor,
            entries: self.entries.iter().skip(offset).cloned().collect(),
        })
    }

    /// Release records through `cursor`. With several readers the owner must acknowledge
    /// their oldest required position, not whichever reader happened to finish first.
    pub(crate) fn acknowledge(&mut self, cursor: ControlCursor) -> Result<(), ControlLogError> {
        let count = self.offset(cursor)?;
        self.entries.drain(..count);
        Ok(())
    }

    pub(crate) fn reset(&mut self) {
        self.cursor = ControlCursor {
            epoch: Uuid::new_v4(),
            sequence: 0,
        };
        self.entries.clear();
    }

    fn offset(&self, cursor: ControlCursor) -> Result<usize, ControlLogError> {
        if cursor.epoch != self.cursor.epoch {
            return Err(ControlLogError::WrongEpoch);
        }
        if cursor.sequence > self.cursor.sequence {
            return Err(ControlLogError::FutureCursor);
        }
        let oldest = self.cursor.sequence - self.entries.len() as u64;
        if cursor.sequence < oldest {
            return Err(ControlLogError::HistoryLost);
        }
        Ok((cursor.sequence - oldest) as usize)
    }
}

#[cfg(test)]
mod tests;
