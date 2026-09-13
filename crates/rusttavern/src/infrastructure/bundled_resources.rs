use crate::infrastructure::assets::ResourceRoots;
use tt_domain::errors::DomainError;
use tt_ports::bundled_template::BundledTemplateStore;

/// Bundled template store backed by the physical resources root.
#[derive(Clone)]
pub(crate) struct BundledResourceStore {
    resources: ResourceRoots,
}

impl BundledResourceStore {
    pub(crate) fn new(resources: ResourceRoots) -> Self {
        Self { resources }
    }
}

impl BundledTemplateStore for BundledResourceStore {
    fn read_text(&self, relative_path: &str) -> Result<String, DomainError> {
        self.resources.read_text(relative_path)
    }
}
