//! Derived native family inputs bound to one immutable frame/lane. Never recording input.
//!
//! The per-instance channel table started as Position-only (hence its name) and now carries every
//! family's fitted writes (TL-548 C2, `native_family_projection`). Frozen Position holds reuse the
//! same input type.
use crate::{
    CapturedFrameToken, EngineError, FamilyNativeWrite, PreparedOutputFrame,
    PreparedStaticFamilyFrame,
};
use light_core::AttributeValue;
use light_core::{AttributeKey, FixtureId, programming::ProgrammingOwner};
use rustc_hash::FxHashMap;
use std::collections::HashSet;
use uuid::Uuid;

/// One full-width fitted Position control on a physical root or copy. Channel/function
/// identities and complete geometry-derived ownership are validated against the capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionNativeWrite {
    pub target: FixtureId,
    pub instance_id: Uuid,
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub function_id: Option<Uuid>,
    pub split: u16,
    pub raw: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct NativePositionInput {
    pub(crate) value: AttributeValue,
    pub(crate) function_id: Option<Uuid>,
    pub(crate) frozen: bool,
    pub(crate) full_freeze: bool,
    /// Family whose fitted write installed this control; `None` for a frozen Position hold.
    pub(crate) family: Option<ProgrammingOwner>,
}
impl NativePositionInput {
    pub(crate) fn frozen(raw: u32, full: bool) -> Self {
        Self {
            value: AttributeValue::RawDmxExact(raw),
            function_id: None,
            frozen: true,
            full_freeze: full,
            family: None,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct NativePositionInstance {
    pub(crate) root: FixtureId,
    pub(crate) channels: Box<[Option<NativePositionInput>]>,
    /// Every (target, family, channel) that wrote this instance, including agreeing shared claims.
    pub(crate) claims: Vec<(FixtureId, ProgrammingOwner, usize)>,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct NativePositionProjection {
    /// Shared by clones (TL-639 round 4): an unchanged collection is installed again by
    /// reference (`FamilyNativeMemo`). Only a fresh candidate is ever written.
    pub(crate) instances: std::sync::Arc<FxHashMap<Uuid, NativePositionInstance>>,
}
impl NativePositionProjection {
    pub(crate) fn instance(
        &self,
        root: FixtureId,
        instance: Uuid,
    ) -> Option<&[Option<NativePositionInput>]> {
        self.instances
            .get(&instance)
            .filter(|row| row.root == root)
            .map(|row| row.channels.as_ref())
    }

    /// Preload ownership mask of one root fixture: the scalar dependency mask of previewed
    /// addresses plus the native writes of previewed family owners. `None` when nothing is owned.
    pub(crate) fn preview_ownership(
        &self,
        projection: &crate::FixtureProjectionPlan,
        root: FixtureId,
        previewed: &HashSet<(FixtureId, AttributeKey)>,
        output: &crate::ResolvedProfileFixtureOutput,
        values: &crate::ResolvedValues,
    ) -> Option<Box<[bool]>> {
        if previewed.is_empty() {
            return None;
        }
        let scalar = projection.native_ownership(
            previewed,
            &output.color_writes,
            values,
            &output.native_active_attributes,
        );
        self.extend_ownership(root, root.0, previewed, scalar)
    }

    /// Merge the native writes of previewed owners into a Preload ownership mask. A fitted family
    /// write owns exactly the controls it installed for `(target, family)` when that owner address
    /// is previewed; the scalar dependency mask is kept. Returns `None` when nothing is owned.
    pub(crate) fn extend_ownership(
        &self,
        root: FixtureId,
        instance: Uuid,
        previewed: &HashSet<(FixtureId, AttributeKey)>,
        mask: Option<Box<[bool]>>,
    ) -> Option<Box<[bool]>> {
        let Some(row) = self.instances.get(&instance).filter(|row| row.root == root) else {
            return mask;
        };
        let mut owned = mask.map_or_else(|| vec![false; row.channels.len()], Vec::from);
        for (target, family, index) in &row.claims {
            if previewed.contains(&(*target, family.key())) {
                owned[*index] = true;
            }
        }
        owned.iter().any(|v| *v).then(|| owned.into_boxed_slice())
    }
}

impl PreparedStaticFamilyFrame {
    /// Position-only form of [`Self::project_family_native`], kept for existing callers: the
    /// whole collection is one complete Position cohort and counts as this frame's installation.
    pub fn project_position_native(
        &mut self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        writes: &[PositionNativeWrite],
    ) -> Result<(), EngineError> {
        let writes: Vec<_> = writes
            .iter()
            .copied()
            .map(FamilyNativeWrite::from)
            .collect();
        self.project_family_native(capture, token, &writes)
    }
}

impl From<PositionNativeWrite> for FamilyNativeWrite {
    fn from(write: PositionNativeWrite) -> Self {
        Self {
            owner: ProgrammingOwner::Position,
            target: write.target,
            instance_id: write.instance_id,
            channel_index: write.channel_index,
            channel_id: write.channel_id,
            function_id: write.function_id,
            split: write.split,
            raw: write.raw,
        }
    }
}
