#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationSurface {
    WatchTui,
    RoundCompanion,
    RoundPreviewLab,
    MenubarPopover,
    PreviewLabArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyProjection {
    pub surface: PresentationSurface,
    pub source_names_visible: bool,
    pub exact_counts_visible: bool,
    pub diagnostic_text_visible: bool,
    pub feed_rows_visible: bool,
    pub file_paths_visible: bool,
    pub project_names_visible: bool,
}

impl PrivacyProjection {
    pub fn for_surface(surface: PresentationSurface) -> Self {
        match surface {
            PresentationSurface::WatchTui => Self {
                surface,
                source_names_visible: true,
                exact_counts_visible: true,
                diagnostic_text_visible: true,
                feed_rows_visible: true,
                file_paths_visible: false,
                project_names_visible: false,
            },
            PresentationSurface::MenubarPopover => Self {
                surface,
                source_names_visible: true,
                exact_counts_visible: true,
                diagnostic_text_visible: false,
                feed_rows_visible: false,
                file_paths_visible: false,
                project_names_visible: false,
            },
            PresentationSurface::RoundCompanion
            | PresentationSurface::RoundPreviewLab
            | PresentationSurface::PreviewLabArtifact => Self::sanitized(surface),
        }
    }

    fn sanitized(surface: PresentationSurface) -> Self {
        Self {
            surface,
            source_names_visible: false,
            exact_counts_visible: false,
            diagnostic_text_visible: false,
            feed_rows_visible: false,
            file_paths_visible: false,
            project_names_visible: false,
        }
    }
}
