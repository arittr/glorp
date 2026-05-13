use crate::tui::style::tokenpet_palette;
use ratatui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Insets {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Insets {
    pub const fn all(value: u16) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn horizontal(value: u16) -> Self {
        Self {
            top: 0,
            right: value,
            bottom: 0,
            left: value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Base,
    Elevated,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTone {
    Primary,
    Label,
    Subtle,
    Accent,
    Good,
    Bad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderTone {
    None,
    Subtle,
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientToken {
    Xp,
    Good,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentStyle {
    surface: Surface,
    text: TextTone,
    border: BorderTone,
    padding: Insets,
    gradient: Option<GradientToken>,
}

impl ComponentStyle {
    pub const fn new() -> Self {
        Self {
            surface: Surface::Empty,
            text: TextTone::Primary,
            border: BorderTone::None,
            padding: Insets::all(0),
            gradient: None,
        }
    }

    pub const fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub const fn text(mut self, text: TextTone) -> Self {
        self.text = text;
        self
    }

    pub const fn border(mut self, border: BorderTone) -> Self {
        self.border = border;
        self
    }

    pub const fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub const fn gradient(mut self, gradient: GradientToken) -> Self {
        self.gradient = Some(gradient);
        self
    }

    pub const fn gradient_token(self) -> Option<GradientToken> {
        self.gradient
    }

    pub fn surface_style(self) -> Style {
        let p = tokenpet_palette();
        match self.surface {
            Surface::Base => Style::default().bg(p.bg.rgb),
            Surface::Elevated => Style::default().bg(p.surface.rgb),
            Surface::Empty => Style::default(),
        }
    }

    pub fn text_style(self) -> Style {
        let p = tokenpet_palette();
        match self.text {
            TextTone::Primary => Style::default().fg(p.fg.rgb),
            TextTone::Label => Style::default().fg(p.dim.rgb),
            TextTone::Subtle => Style::default().fg(p.faint.rgb),
            TextTone::Accent => Style::default().fg(p.accent.rgb),
            TextTone::Good => Style::default().fg(p.good.rgb),
            TextTone::Bad => Style::default().fg(p.bad.rgb),
        }
    }

    pub fn border_style(self) -> Style {
        let p = tokenpet_palette();
        match self.border {
            BorderTone::None => Style::default(),
            BorderTone::Subtle => Style::default().fg(p.faint.rgb),
            BorderTone::Accent => Style::default().fg(p.accent.rgb),
        }
    }

    pub const fn insets(self) -> Insets {
        self.padding
    }
}

impl Default for ComponentStyle {
    fn default() -> Self {
        Self::new()
    }
}
