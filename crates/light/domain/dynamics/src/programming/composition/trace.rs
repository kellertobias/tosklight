//! Runtime-only lineage for one family composition. The graph records the same field writes
//! that produced the value; it does not guess destination native channel ownership.
use super::FamilySampleRank;
use light_core::programming::{
    ProgrammingComponent, ProgrammingFieldScope, ProgrammingTraceField, ProgrammingTransitionTrace,
};

fn covered_fields(
    footprint: FamilyTraceFootprint,
    fields: &ProgrammingFieldScope,
) -> ProgrammingFieldScope {
    match footprint {
        FamilyTraceFootprint::Whole => fields.clone(),
        FamilyTraceFootprint::Component(component) => {
            fields.intersection(&ProgrammingFieldScope::from_component(component))
        }
    }
}

fn recipe_inputs() -> ProgrammingFieldScope {
    use ProgrammingTraceField as F;
    ProgrammingFieldScope::new([
        F::ColorRecipeRed,
        F::ColorRecipeGreen,
        F::ColorRecipeBlue,
        F::ColorRecipeAmber,
    ])
}

/// A base-color component edit recalculates XYZ from the complete recipe. HSV edits also
/// recalculate all RGB channels from their original RGB inputs. These are input dependencies,
/// not extra authored footprints attached to the incoming component.
fn write_inputs(
    footprint: FamilyTraceFootprint,
    requested: &ProgrammingFieldScope,
    partial: bool,
) -> Option<(
    ProgrammingFieldScope,
    ProgrammingFieldScope,
    ProgrammingFieldScope,
)> {
    use ProgrammingTraceField as F;
    use light_core::programming::ColorComponent as C;
    if matches!(
        footprint,
        FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(_))
    ) && requested.contains(F::NativePrediction)
    {
        // Component composition predicts the complete edited recipe. The predictor does
        // not supply input-field evidence, so the old portable estimate is not an author
        // of this new prediction. Raw channel writes retain their exact ordinary trace.
        return None;
    }
    let covered = covered_fields(footprint, requested);
    let mut prior = if partial {
        requested.clone()
    } else {
        requested.difference(&covered).ok()?
    };
    let mut dependencies = ProgrammingFieldScope::empty();
    match footprint {
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(
            C::Red | C::Green | C::Blue | C::Amber,
        )) if requested.contains(F::ColorXyz) => {
            let untouched_recipe = recipe_inputs()
                .difference(&covered_fields(footprint, &recipe_inputs()))
                .ok()?;
            prior = prior.union(&untouched_recipe);
        }
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(C::Hue | C::Saturation))
            if !covered.is_empty() =>
        {
            // Hue/Saturation preserve the other HSV dimensions and brightness from RGB.
            dependencies = ProgrammingFieldScope::new([
                F::ColorRecipeRed,
                F::ColorRecipeGreen,
                F::ColorRecipeBlue,
            ]);
            if requested.contains(F::ColorXyz) {
                prior = prior.union(&ProgrammingFieldScope::new([F::ColorRecipeAmber]));
            }
        }
        _ => {}
    }
    Some((covered, prior, dependencies))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FamilyTraceNodeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyTraceFootprint {
    Whole,
    Component(ProgrammingComponent),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyTraceRole {
    Authored,
    /// A generated Current partner, the original pre-Dynamic family, or a live frame input.
    CalculationDependency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyTraceSource {
    pub rank: FamilySampleRank,
    pub footprint: FamilyTraceFootprint,
    pub role: FamilyTraceRole,
    /// Exact immutable captured assignment, independent of rank and value. None is Unknown.
    pub occurrence: Option<crate::DynamicSourceOccurrenceId>,
}

/// Exact source and the input fields read at that source's leaf. Field conversions can make
/// these differ from the originally requested output fields without changing authorship.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyTraceContribution {
    pub source: FamilyTraceSource,
    pub fields: ProgrammingFieldScope,
}

/// Detailed query over the matching composed value. Baseline scopes refer to the original
/// captured family, after reversing every intervening field transfer. They carry no invented
/// Dynamic rank or identity; the caller resolves them against its captured static evidence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FamilyTraceQuery {
    pub sources: Vec<FamilyTraceContribution>,
    /// Baseline fields which remain ordinary contributors to the requested output.
    pub base_fields: ProgrammingFieldScope,
    /// Baseline fields read as calculation dependencies. A field can occur in both scopes
    /// when separate paths use it in different roles; neither relationship replaces the other.
    pub base_dependency_fields: ProgrammingFieldScope,
}

/// Arbitration coverage is independent from the fields used to calculate appearance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyControlContribution {
    pub rank: FamilySampleRank,
    pub footprint: FamilyTraceFootprint,
    pub fields: ProgrammingFieldScope,
}

#[derive(Clone, Debug)]
pub(super) enum FamilyTraceLeaf {
    Source(FamilyTraceSource),
    Current(FamilyTraceSource, crate::DynamicSourceDependency),
}

#[derive(Clone, Debug)]
enum Node {
    Base,
    Source(FamilyTraceSource),
    /// The source's original footprint describes its Dynamic lane, while the transfer
    /// maps that lane's output back to the captured static family's input fields.
    Current(FamilyTraceSource, crate::DynamicSourceTransfer),
    /// A complete owner/axis bundle has one arbitration rank but may have several original
    /// authored and Current leaves. Each leaf retains its own field footprint.
    Bundle(Vec<FamilyTraceNodeId>),
    /// Full writes cut the old value only in `footprint`; partial writes keep it as underlay.
    Write {
        prior: FamilyTraceNodeId,
        incoming: FamilyTraceNodeId,
        footprint: FamilyTraceFootprint,
        partial: bool,
    },
    /// Retained transitions/Size retain both inputs only when both actually participate.
    Blend {
        from: FamilyTraceNodeId,
        to: FamilyTraceNodeId,
    },
    /// Field transfer follows the same operation that produced the value. None is an unknown
    /// transfer (for example a frame conversion without explicit evidence), not an empty blend.
    MappedBlend {
        from: FamilyTraceNodeId,
        to: FamilyTraceNodeId,
        transfer: Option<ProgrammingTransitionTrace>,
    },
    Dependency(FamilyTraceNodeId),
    Control {
        rank: FamilySampleRank,
        footprint: FamilyTraceFootprint,
        appearance: FamilyTraceNodeId,
        prior: Option<FamilyTraceNodeId>,
    },
}

/// Reused by the compositor. A trace is meaningful only for the matching composed value and
/// coherent frame. Node IDs remain valid until `clear` starts the next composition.
#[derive(Default)]
pub struct FamilyTraceArena {
    nodes: Vec<Node>,
    root: Option<FamilyTraceNodeId>,
}

impl FamilyTraceArena {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.root = None;
    }

    pub fn root(&self) -> Option<FamilyTraceNodeId> {
        self.root
    }

    pub(super) fn set_root(&mut self, root: FamilyTraceNodeId) {
        self.root = Some(root);
    }

    fn push(&mut self, node: Node) -> FamilyTraceNodeId {
        let id = FamilyTraceNodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub(super) fn base(&mut self) -> FamilyTraceNodeId {
        self.push(Node::Base)
    }

    pub(super) fn source(&mut self, source: FamilyTraceSource) -> FamilyTraceNodeId {
        self.push(Node::Source(source))
    }

    pub(super) fn current_source(
        &mut self,
        mut source: FamilyTraceSource,
        dependency: crate::DynamicSourceDependency,
    ) -> FamilyTraceNodeId {
        source.occurrence = dependency.occurrence;
        source.role = FamilyTraceRole::CalculationDependency;
        self.push(Node::Current(source, dependency.transfer))
    }

    pub(super) fn leaf(&mut self, leaf: FamilyTraceLeaf) -> FamilyTraceNodeId {
        match leaf {
            FamilyTraceLeaf::Source(source) => self.source(source),
            FamilyTraceLeaf::Current(source, dependency) => self.current_source(source, dependency),
        }
    }

    pub(super) fn bundle(&mut self, leaves: Vec<FamilyTraceNodeId>) -> FamilyTraceNodeId {
        self.push(Node::Bundle(leaves))
    }

    pub(super) fn write(
        &mut self,
        prior: FamilyTraceNodeId,
        incoming: FamilyTraceNodeId,
        footprint: FamilyTraceFootprint,
        partial: bool,
    ) -> FamilyTraceNodeId {
        self.push(Node::Write {
            prior,
            incoming,
            footprint,
            partial,
        })
    }

    pub(super) fn blend(
        &mut self,
        from: FamilyTraceNodeId,
        to: FamilyTraceNodeId,
    ) -> FamilyTraceNodeId {
        self.push(Node::Blend { from, to })
    }

    pub(super) fn mapped_blend(
        &mut self,
        from: FamilyTraceNodeId,
        to: FamilyTraceNodeId,
        transfer: Option<ProgrammingTransitionTrace>,
    ) -> FamilyTraceNodeId {
        self.push(Node::MappedBlend { from, to, transfer })
    }

    pub(super) fn control(
        &mut self,
        rank: FamilySampleRank,
        footprint: FamilyTraceFootprint,
        appearance: FamilyTraceNodeId,
        prior: Option<FamilyTraceNodeId>,
    ) -> FamilyTraceNodeId {
        self.push(Node::Control {
            rank,
            footprint,
            appearance,
            prior,
        })
    }

    /// Winning control coverage, including a Current vote at endpoint master zero. These
    /// ranks are control ownership, not authored appearance or captured Current identity.
    pub fn control_sources_for_fields(
        &self,
        root: FamilyTraceNodeId,
        fields: &ProgrammingFieldScope,
    ) -> Option<Vec<FamilyControlContribution>> {
        let mut result: Vec<FamilyControlContribution> = Vec::new();
        let mut pending = vec![(root, fields.clone())];
        while let Some((id, requested)) = pending.pop() {
            if requested.is_empty() {
                continue;
            }
            let mut add = |rank, footprint| {
                let fields = covered_fields(footprint, &requested);
                if fields.is_empty() {
                    return;
                }
                if let Some(known) = result
                    .iter_mut()
                    .find(|entry| entry.rank == rank && entry.footprint == footprint)
                {
                    known.fields = known.fields.union(&fields);
                } else {
                    result.push(FamilyControlContribution {
                        rank,
                        footprint,
                        fields,
                    });
                }
            };
            match self.nodes.get(id.0)? {
                Node::Control {
                    rank,
                    footprint,
                    prior,
                    ..
                } => {
                    add(*rank, *footprint);
                    if let Some(prior) = prior {
                        pending.push((*prior, requested));
                    }
                }
                Node::Source(source) if source.role == FamilyTraceRole::Authored => {
                    add(source.rank, source.footprint)
                }
                Node::Base | Node::Source(_) | Node::Current(..) | Node::Dependency(_) => {}
                Node::Bundle(leaves) => {
                    pending.extend(leaves.iter().map(|id| (*id, requested.clone())))
                }
                Node::Write {
                    prior,
                    incoming,
                    footprint,
                    partial,
                } => {
                    let covered = covered_fields(*footprint, &requested);
                    let remaining = if *partial {
                        requested.clone()
                    } else {
                        requested.difference(&covered).ok()?
                    };
                    pending.push((*prior, remaining));
                    pending.push((*incoming, covered));
                }
                Node::Blend { from, to } | Node::MappedBlend { from, to, .. } => {
                    pending.push((*from, requested.clone()));
                    pending.push((*to, requested));
                }
            }
        }
        Some(result)
    }

    pub(super) fn dependency(&mut self, input: FamilyTraceNodeId) -> FamilyTraceNodeId {
        self.push(Node::Dependency(input))
    }

    /// A nested composition starts with a synthetic Base node. Bind that node to the exact
    /// lower-prefix trace captured by its enclosing composition before importing its writes.
    /// This preserves older authored sources only where the nested writes actually read them.
    pub(super) fn append_graph_rebased(
        &mut self,
        other: &FamilyTraceArena,
        root: FamilyTraceNodeId,
        base: FamilyTraceNodeId,
    ) -> FamilyTraceNodeId {
        let mut ids = Vec::with_capacity(other.nodes.len());
        for node in &other.nodes {
            let mapped = match node {
                Node::Base => base,
                Node::Source(source) => self.source(*source),
                Node::Current(source, transfer) => {
                    self.push(Node::Current(*source, transfer.clone()))
                }
                Node::Bundle(leaves) => self.bundle(leaves.iter().map(|id| ids[id.0]).collect()),
                Node::Write {
                    prior,
                    incoming,
                    footprint,
                    partial,
                } => self.write(ids[prior.0], ids[incoming.0], *footprint, *partial),
                Node::Blend { from, to } => self.blend(ids[from.0], ids[to.0]),
                Node::MappedBlend { from, to, transfer } => {
                    self.mapped_blend(ids[from.0], ids[to.0], transfer.clone())
                }
                Node::Dependency(input) => self.dependency(ids[input.0]),
                Node::Control {
                    rank,
                    footprint,
                    appearance,
                    prior,
                } => self.control(
                    *rank,
                    *footprint,
                    ids[appearance.0],
                    prior.map(|id| ids[id.0]),
                ),
            };
            ids.push(mapped);
        }
        ids[root.0]
    }

    /// Compatibility query for a component's actual fields. None means a participating
    /// operation lacks transfer evidence; Some(empty) proves no traced source supplied them.
    pub fn sources_for_component(
        &self,
        root: FamilyTraceNodeId,
        component: ProgrammingComponent,
    ) -> Option<Vec<FamilyTraceSource>> {
        use ProgrammingTraceField as F;
        use light_core::programming::ColorComponent;
        // A recipe component reads its recorded recipe field. Its write scope additionally
        // contains derived XYZ, which must not turn an unchanged Green read into a Red read.
        let fields = match component {
            ProgrammingComponent::Color(ColorComponent::Red) => {
                ProgrammingFieldScope::new([F::ColorRecipeRed])
            }
            ProgrammingComponent::Color(ColorComponent::Green) => {
                ProgrammingFieldScope::new([F::ColorRecipeGreen])
            }
            ProgrammingComponent::Color(ColorComponent::Blue) => {
                ProgrammingFieldScope::new([F::ColorRecipeBlue])
            }
            ProgrammingComponent::Color(ColorComponent::Amber) => {
                ProgrammingFieldScope::new([F::ColorRecipeAmber])
            }
            ProgrammingComponent::Color(ColorComponent::Hue | ColorComponent::Saturation) => {
                ProgrammingFieldScope::new([
                    F::ColorRecipeRed,
                    F::ColorRecipeGreen,
                    F::ColorRecipeBlue,
                ])
            }
            _ => ProgrammingFieldScope::from_component(component),
        };
        self.sources_for_fields(root, &fields)
    }

    pub fn sources_for_field(
        &self,
        root: FamilyTraceNodeId,
        field: ProgrammingTraceField,
    ) -> Option<Vec<FamilyTraceSource>> {
        self.sources_for_fields(root, &ProgrammingFieldScope::new([field]))
    }

    /// Follow requested output fields back through every transfer to the original input fields.
    /// The returned source keeps its authored footprint even when a conversion remaps that
    /// footprint into another representation. Unknown transfers never become known empty sets.
    pub fn sources_for_fields(
        &self,
        root: FamilyTraceNodeId,
        fields: &ProgrammingFieldScope,
    ) -> Option<Vec<FamilyTraceSource>> {
        self.query_fields(root, fields)
            .map(|entries| entries.into_iter().map(|entry| entry.source).collect())
    }

    /// Source-only query retaining each source's transformed input scope. For the surviving
    /// original static baseline as well, use `query_fields_with_base`.
    pub fn query_fields(
        &self,
        root: FamilyTraceNodeId,
        fields: &ProgrammingFieldScope,
    ) -> Option<Vec<FamilyTraceContribution>> {
        self.query_fields_with_base(root, fields)
            .map(|query| query.sources)
    }

    /// Lossless query retaining transformed input fields for explicit sources and the original
    /// captured baseline. None means an unknown transfer; an empty result is known absence.
    /// Repeated visits union input scopes within each source or baseline relationship.
    pub fn query_fields_with_base(
        &self,
        root: FamilyTraceNodeId,
        fields: &ProgrammingFieldScope,
    ) -> Option<FamilyTraceQuery> {
        let mut query = FamilyTraceQuery::default();
        let mut pending = vec![(root, false, fields.clone())];
        while let Some((id, dependency, requested)) = pending.pop() {
            if requested.is_empty() {
                continue;
            }
            let node = self.nodes.get(id.0)?;
            match node {
                Node::Base => {
                    let scope = if dependency {
                        &mut query.base_dependency_fields
                    } else {
                        &mut query.base_fields
                    };
                    *scope = scope.union(&requested);
                }
                Node::Source(source)
                    if !covered_fields(source.footprint, &requested).is_empty() =>
                {
                    let mut source = *source;
                    if dependency {
                        source.role = FamilyTraceRole::CalculationDependency;
                    }
                    let fields = covered_fields(source.footprint, &requested);
                    if let Some(known) = query
                        .sources
                        .iter_mut()
                        .find(|entry| entry.source == source)
                    {
                        known.fields = known.fields.union(&fields);
                    } else {
                        query
                            .sources
                            .push(FamilyTraceContribution { source, fields });
                    }
                }
                Node::Source(_) => {}
                Node::Current(source, transfer) => {
                    let outputs = covered_fields(source.footprint, &requested);
                    if outputs.is_empty() {
                        continue;
                    }
                    let fields = match transfer {
                        crate::DynamicSourceTransfer::Identity => outputs,
                        crate::DynamicSourceTransfer::Mapped(transfer) => {
                            transfer.reverse(&outputs)
                        }
                        crate::DynamicSourceTransfer::Unknown => return None,
                    };
                    if fields.is_empty() {
                        continue;
                    }
                    // Do not clip input fields back to the lane footprint: Target->Pan and
                    // recipe->XYZ conversions intentionally cross representation fields.
                    if let Some(known) = query
                        .sources
                        .iter_mut()
                        .find(|entry| entry.source == *source)
                    {
                        known.fields = known.fields.union(&fields);
                    } else {
                        query.sources.push(FamilyTraceContribution {
                            source: *source,
                            fields,
                        });
                    }
                }
                Node::Bundle(leaves) => pending.extend(
                    leaves
                        .iter()
                        .rev()
                        .map(|id| (*id, dependency, requested.clone())),
                ),
                Node::Write {
                    prior,
                    incoming,
                    footprint,
                    partial,
                } => {
                    let (covered, remaining, dependencies) =
                        write_inputs(*footprint, &requested, *partial)?;
                    pending.push((*prior, dependency, remaining));
                    pending.push((*prior, true, dependencies));
                    pending.push((*incoming, dependency, covered));
                }
                Node::Blend { from, to } => {
                    pending.push((*from, dependency, requested.clone()));
                    pending.push((*to, dependency, requested));
                }
                Node::MappedBlend { from, to, transfer } => {
                    let transfer = transfer.as_ref()?;
                    pending.push((*from, dependency, transfer.from.reverse(&requested)));
                    pending.push((*to, dependency, transfer.to.reverse(&requested)));
                }
                Node::Dependency(input) => pending.push((*input, true, requested)),
                Node::Control { appearance, .. } => {
                    pending.push((*appearance, dependency, requested))
                }
            }
        }
        Some(query)
    }
}

#[cfg(test)]
mod tests;
