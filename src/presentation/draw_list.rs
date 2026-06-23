/// A single resolved cell to write during blitting. All coordinates are
/// absolute (col, row) in the terminal buffer. Fields are optional so callers
/// can emit bg-only, glyph-only, or fully-styled cells without touching the
/// fields they don't own — the blitter must NEVER reset fields that are `None`.
///
/// Colors are backend-agnostic [`crate::pet::palette::Rgb`] — no ratatui
/// imports here so the AppKit companion (Plan 06) can consume the same list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawCell {
    pub row: u16,
    pub col: u16,
    /// Terminal character to place. `None` leaves the existing symbol intact.
    pub glyph: Option<String>,
    /// Foreground color. `None` leaves the existing fg intact.
    pub fg: Option<crate::pet::palette::Rgb>,
    /// Background color. `None` leaves the existing bg intact.
    pub bg: Option<crate::pet::palette::Rgb>,
    /// Whether to add the BOLD modifier. `false` leaves modifiers intact.
    pub bold: bool,
}

/// An ordered list of resolved draw cells for one scene pass. The blitter
/// writes them in order, so later entries in the list win over earlier ones.
#[derive(Debug, Clone, Default)]
pub struct SceneDrawList {
    pub cells: Vec<DrawCell>,
}

impl SceneDrawList {
    pub fn push(&mut self, cell: DrawCell) {
        self.cells.push(cell);
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = DrawCell>) {
        self.cells.extend(iter);
    }
}
