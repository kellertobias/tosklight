use super::Planner;
use crate::selective_import::{
    ImportBlocker, ImportDependency, ImportDependencyDisposition, ImportObjectDescriptor,
};
use light_show::PortableShowObjectKey;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct DependencyKey {
    owner: PortableShowObjectKey,
    dependency: PortableShowObjectKey,
    disposition: u8,
}

impl<P: crate::selective_import::SelectiveShowImportPorts> Planner<'_, P> {
    pub(super) fn visit_references(
        &mut self,
        owner: &PortableShowObjectKey,
        descriptor: &ImportObjectDescriptor,
    ) {
        for reference in &descriptor.references {
            self.visit_dependency(owner, reference.target.clone());
        }
    }

    pub(super) fn visit_dependency(
        &mut self,
        owner: &PortableShowObjectKey,
        dependency: PortableShowObjectKey,
    ) {
        let disposition = self.dependency_disposition(owner, &dependency);
        self.dependencies.insert(DependencyKey {
            owner: owner.clone(),
            dependency,
            disposition,
        });
    }

    fn dependency_disposition(
        &mut self,
        owner: &PortableShowObjectKey,
        dependency: &PortableShowObjectKey,
    ) -> u8 {
        if self
            .source
            .object(dependency.kind(), dependency.id())
            .is_some()
        {
            self.pending.insert(dependency.clone());
            return if self.request.selected_objects.contains(dependency) {
                0
            } else {
                1
            };
        }
        if self
            .target
            .object(dependency.kind(), dependency.id())
            .is_some()
        {
            self.bind_destination(dependency);
            return 2;
        }
        self.blockers.push(ImportBlocker::MissingObject {
            key: dependency.clone(),
            required_by: Some(owner.clone()),
        });
        3
    }
}

impl From<DependencyKey> for ImportDependency {
    fn from(value: DependencyKey) -> Self {
        Self {
            owner: value.owner,
            dependency: value.dependency,
            disposition: match value.disposition {
                0 => ImportDependencyDisposition::Selected,
                1 => ImportDependencyDisposition::Included,
                2 => ImportDependencyDisposition::BoundToDestination,
                _ => ImportDependencyDisposition::Missing,
            },
        }
    }
}
