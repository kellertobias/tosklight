use super::{
    ColorAuthoringModel, ColorIntent, FamilyEditContext, JointAngles, NativeColorEditModel,
    VirtualColorAuthoringV1,
};
use std::sync::Arc;

/// Immutable adoption inputs retained by a runtime gesture or Align anchor. Models must be
/// immutable snapshots and must never consult live engine/profile state. This context is not
/// serialized into intents, Programmer Undo snapshots or show data.
#[derive(Clone)]
pub struct OwnedFamilyEditContext {
    pub solved_angles: Option<JointAngles>,
    /// A physical adapter attempted capture for this edit. Missing pose/seed after an
    /// attempt is expected unavailability, distinct from an incomplete caller environment.
    pub position_adoption_attempted: bool,
    pub semantic_color_adoption: Option<ColorIntent>,
    pub color_model: Option<Arc<dyn ColorAuthoringModel + Send + Sync>>,
    pub native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    /// TL-554: the complete Direct value captured once from the reference head's published
    /// premaster output by the first real native edit of a gesture. A native edit replaces a
    /// seed that is not already Direct of `native_model`'s source with it; an existing Direct
    /// value of that source is edited in place and never reseeded.
    pub direct_color_seed: Option<crate::AttributeValue>,
}

impl Default for OwnedFamilyEditContext {
    fn default() -> Self {
        Self {
            solved_angles: None,
            position_adoption_attempted: false,
            semantic_color_adoption: None,
            color_model: Some(Arc::new(VirtualColorAuthoringV1)),
            native_model: None,
            direct_color_seed: None,
        }
    }
}

impl std::fmt::Debug for OwnedFamilyEditContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedFamilyEditContext")
            .field("solved_angles", &self.solved_angles)
            .field(
                "position_adoption_attempted",
                &self.position_adoption_attempted,
            )
            .field("semantic_color_adoption", &self.semantic_color_adoption)
            .field("has_color_model", &self.color_model.is_some())
            .field("has_native_model", &self.native_model.is_some())
            .field("direct_color_seed", &self.direct_color_seed)
            .finish()
    }
}

impl PartialEq for OwnedFamilyEditContext {
    fn eq(&self, other: &Self) -> bool {
        fn same<T: ?Sized>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
        }
        // Pointer equality describes runtime capture identity only. Value-change detection
        // compares the complete AttributeValue, never the source model's allocation identity.
        self.solved_angles == other.solved_angles
            && self.position_adoption_attempted == other.position_adoption_attempted
            && self.semantic_color_adoption == other.semantic_color_adoption
            && same(&self.color_model, &other.color_model)
            && same(&self.native_model, &other.native_model)
            && self.direct_color_seed == other.direct_color_seed
    }
}

impl OwnedFamilyEditContext {
    /// The base a family edit starts from. A native Color edit whose seed is not already a
    /// Direct value of the pinned source model starts from the captured Direct seed instead;
    /// every other edit (and a Direct value of that source) keeps its own seed.
    pub fn edit_base<'a>(
        &'a self,
        seed: &'a crate::AttributeValue,
        edits: &[super::ComponentEdit],
    ) -> &'a crate::AttributeValue {
        let native = edits
            .iter()
            .any(|edit| matches!(edit, super::ComponentEdit::Native { .. }));
        let (true, Some(captured)) = (native, self.direct_color_seed.as_ref()) else {
            return seed;
        };
        let in_place = match (seed, self.native_model.as_deref()) {
            (crate::AttributeValue::ColorProgram(program), Some(model)) => matches!(
                program.as_ref(),
                super::ColorProgram::Direct { recipe, .. } if &recipe.source == model.source()
            ),
            _ => false,
        };
        if in_place { seed } else { captured }
    }

    pub fn borrowed(&self) -> FamilyEditContext<'_> {
        FamilyEditContext {
            solved_angles: self.solved_angles,
            semantic_color_adoption: self.semantic_color_adoption.as_ref(),
            color_model: self
                .color_model
                .as_deref()
                .map(|v| v as &dyn ColorAuthoringModel),
            native_model: self
                .native_model
                .as_deref()
                .map(|v| v as &dyn NativeColorEditModel),
        }
    }
}
