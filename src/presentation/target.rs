#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceTargetId(String);

impl SurfaceTargetId {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        assert!(
            !id.starts_with("watch."),
            "presentation target ids must not be watch TargetPath values"
        );
        Self(id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
