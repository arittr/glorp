use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentPath(&'static str);

impl ComponentPath {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ComponentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetPath(&'static str);

impl TargetPath {
    pub const fn new(path: &'static str) -> Self {
        Self(path)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchComponentId {
    Root,
    Pet,
    Vitals,
    Bio,
    Today,
    Progress,
    Feed,
}

impl WatchComponentId {
    pub const fn path(self) -> ComponentPath {
        match self {
            WatchComponentId::Root => ComponentPath::new("watch"),
            WatchComponentId::Pet => ComponentPath::new("watch.pet"),
            WatchComponentId::Vitals => ComponentPath::new("watch.vitals"),
            WatchComponentId::Bio => ComponentPath::new("watch.bio"),
            WatchComponentId::Today => ComponentPath::new("watch.today"),
            WatchComponentId::Progress => ComponentPath::new("watch.progress"),
            WatchComponentId::Feed => ComponentPath::new("watch.feed"),
        }
    }
}
