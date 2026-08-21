//! One frame's values, as everything outside the engine sees them.
//!
//! The render resolves into slots. Persistence, the HTTP and WebSocket boundary, OSC, and the
//! operator all speak in names, so a map has to exist somewhere — but it does not have to exist
//! every frame. Most frames are never asked for one: the output scheduler point-queries a handful
//! of values, and the visualization publisher only stores a handle that a client may or may not
//! ever read.
//!
//! So the map is built on the first ask and never rebuilt, and a caller who wants one value asks
//! for one value and no map is built at all.

use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId};

use crate::contribution::ResolvedFrame;
use crate::{ResolvedChangedAt, ResolvedValues};

struct FrameShared {
    /// The dense frame, when it holds everything the show resolved. Absent when something the
    /// compiled patch could not number overflowed, or when the values were handed in from outside
    /// rather than resolved here.
    frame: Option<ResolvedFrame>,
    /// Everything, when there is no frame to read.
    fallback_values: ResolvedValues,
    fallback_changed_at: ResolvedChangedAt,
    values: OnceLock<ResolvedValues>,
    changed_at: OnceLock<ResolvedChangedAt>,
}

/// A frame's values by name, materialised only if someone asks for all of them.
///
/// Cheap to clone: a clone shares the frame and whatever map has already been built, so handing
/// this to a consumer costs a reference count rather than a copy of the show.
#[derive(Clone)]
pub struct FrameValues {
    shared: Arc<FrameShared>,
}

impl FrameValues {
    pub(crate) fn from_frame(frame: ResolvedFrame) -> Self {
        Self {
            shared: Arc::new(FrameShared {
                frame: Some(frame),
                fallback_values: ResolvedValues::default(),
                fallback_changed_at: ResolvedChangedAt::default(),
                values: OnceLock::new(),
                changed_at: OnceLock::new(),
            }),
        }
    }

    /// Values that were not resolved into a frame, or could not all be held by one.
    pub fn from_maps(values: ResolvedValues, changed_at: ResolvedChangedAt) -> Self {
        Self {
            shared: Arc::new(FrameShared {
                frame: None,
                fallback_values: values,
                fallback_changed_at: changed_at,
                values: OnceLock::new(),
                changed_at: OnceLock::new(),
            }),
        }
    }

    /// An empty frame, for a boundary that has to name one before a render has happened.
    pub fn empty() -> Self {
        Self::from_maps(ResolvedValues::default(), ResolvedChangedAt::default())
    }

    /// The dense frame behind these values, when there is one.
    pub(crate) fn frame(&self) -> Option<&ResolvedFrame> {
        self.shared.frame.as_ref()
    }

    /// One value by name, without building a map to find it.
    ///
    /// This is the call for anything on the frame path. Reaching for [`Self::values`] to look up a
    /// single attribute materialises the whole show to answer one question.
    pub fn value(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&AttributeValue> {
        match &self.shared.frame {
            Some(frame) => {
                let slot = frame.slots().slot(fixture_id, attribute)?;
                frame.value(slot)
            }
            None => self
                .shared
                .fallback_values
                .get(&(fixture_id, attribute.clone())),
        }
    }

    /// When one value last changed, without building a map to find out.
    pub fn changed_at(
        &self,
        fixture_id: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<DateTime<Utc>> {
        match &self.shared.frame {
            Some(frame) => {
                let slot = frame.slots().slot(fixture_id, attribute)?;
                frame.changed_at(slot)
            }
            None => self
                .shared
                .fallback_changed_at
                .get(&(fixture_id, attribute.clone()))
                .copied(),
        }
    }

    /// Every value in the frame, by name. Built once, on the first ask.
    pub fn values(&self) -> &ResolvedValues {
        let Some(frame) = self.shared.frame.as_ref() else {
            return &self.shared.fallback_values;
        };
        self.shared.values.get_or_init(|| {
            let mut values =
                ResolvedValues::with_capacity_and_hasher(frame.occupied_len(), Default::default());
            for (slot, winner) in frame.occupied() {
                let (fixture_id, attribute) = frame.slots().pair(slot);
                values.insert((fixture_id, attribute.clone()), winner.value.clone());
            }
            values
        })
    }

    /// Every winning timestamp in the frame, by name. Built once, on the first ask.
    pub fn changed_at_map(&self) -> &ResolvedChangedAt {
        let Some(frame) = self.shared.frame.as_ref() else {
            return &self.shared.fallback_changed_at;
        };
        self.shared.changed_at.get_or_init(|| {
            let mut changed_at = ResolvedChangedAt::with_capacity_and_hasher(
                frame.occupied_len(),
                Default::default(),
            );
            for (slot, winner) in frame.occupied() {
                let (fixture_id, attribute) = frame.slots().pair(slot);
                changed_at.insert((fixture_id, attribute.clone()), winner.changed_at);
            }
            changed_at
        })
    }

    /// How many values the frame holds, without building a map to count them.
    pub fn len(&self) -> usize {
        match &self.shared.frame {
            Some(frame) => frame.occupied_len(),
            None => self.shared.fallback_values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether anything has asked for the whole frame by name yet.
    ///
    /// The output path must not: it point-queries what it needs, and building the map would put a
    /// pass over the show back into every frame. Tests assert this so that cannot come back
    /// quietly.
    pub fn materialised_by_name(&self) -> bool {
        self.shared.values.get().is_some() || self.shared.changed_at.get().is_some()
    }

    /// Whether these values are backed by the dense frame rather than a map.
    pub fn is_dense(&self) -> bool {
        self.shared.frame.is_some()
    }
}

/// Reading a `FrameValues` as the map it stands for. Convenient, and expensive the first time: this
/// is what builds the map. Prefer [`FrameValues::value`] when one value is the question.
impl Deref for FrameValues {
    type Target = ResolvedValues;

    fn deref(&self) -> &Self::Target {
        self.values()
    }
}

impl std::fmt::Debug for FrameValues {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FrameValues")
            .field("len", &self.len())
            .field("dense", &self.shared.frame.is_some())
            .finish()
    }
}
