//! Text styles. A label names a role — its size and colour — instead of a font size.

use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window,
    div,
};

use crate::theme::{ActiveTheme, Theme};

/// A step on the type scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextSize {
    /// Captions, badges, keyboard hints.
    XSmall,
    /// Secondary labels and dense rows.
    Small,
    /// Body text and control labels.
    #[default]
    Medium,
    /// Section headings.
    Large,
    /// Panel and sheet titles.
    XLarge,
    /// Big numbers: tempo, timecode, counters.
    Display,
}

impl TextSize {
    /// The size in pixels under `theme`.
    pub fn pixels(self, theme: &Theme) -> gpui::Pixels {
        let typography = &theme.typography;
        match self {
            Self::XSmall => typography.size_xs,
            Self::Small => typography.size_sm,
            Self::Medium => typography.size_md,
            Self::Large => typography.size_lg,
            Self::XLarge => typography.size_xl,
            Self::Display => typography.size_display,
        }
    }
}

/// A semantic text colour.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextColor {
    /// Primary text.
    #[default]
    Default,
    /// Secondary text.
    Muted,
    /// Text that should recede.
    Faint,
    /// Accent-coloured, readable text.
    Accent,
    /// The AI's identity colour.
    Agent,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Danger.
    Danger,
    /// Disabled.
    Disabled,
    /// An explicit colour.
    Custom(Hsla),
}

impl TextColor {
    /// The colour under `theme`.
    pub fn resolve(self, theme: &Theme) -> Hsla {
        let colors = &theme.colors;
        match self {
            Self::Default => colors.text,
            Self::Muted => colors.text_muted,
            Self::Faint => colors.text_faint,
            Self::Accent => colors.accent_text,
            Self::Agent => colors.agent,
            Self::Success => colors.success,
            Self::Warning => colors.warning,
            Self::Danger => colors.danger,
            Self::Disabled => colors.text_disabled,
            Self::Custom(color) => color,
        }
    }
}

/// How heavy a label is set.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Weight {
    Normal,
    /// The theme's heading weight, which differs between directions.
    Heading,
    Explicit(FontWeight),
}

/// A run of interface text.
#[derive(IntoElement)]
pub struct Label {
    text: SharedString,
    size: TextSize,
    color: TextColor,
    weight: Weight,
    mono: bool,
    truncate: bool,
    uppercase: bool,
}

impl Label {
    /// Body text in the default colour.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            size: TextSize::default(),
            color: TextColor::default(),
            weight: Weight::Normal,
            mono: false,
            truncate: false,
            uppercase: false,
        }
    }

    /// A heading: large, in the theme's heading weight.
    pub fn heading(text: impl Into<SharedString>) -> Self {
        Self::new(text).size(TextSize::Large).heading_weight()
    }

    /// A small uppercase caption, for section titles in sidebars and inspectors.
    pub fn caption(text: impl Into<SharedString>) -> Self {
        let mut label = Self::new(text)
            .size(TextSize::XSmall)
            .color(TextColor::Faint)
            .weight(FontWeight::SEMIBOLD);
        label.uppercase = true;
        label
    }

    /// Step on the type scale.
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// Semantic colour.
    pub fn color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// Explicit weight.
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = Weight::Explicit(weight);
        self
    }

    /// The theme's heading weight.
    pub fn heading_weight(mut self) -> Self {
        self.weight = Weight::Heading;
        self
    }

    /// Set in the monospaced font, for values, code and timecode.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// Cut with an ellipsis instead of wrapping.
    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }
}

impl RenderOnce for Label {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let weight = match self.weight {
            Weight::Normal => FontWeight::NORMAL,
            Weight::Heading => theme.typography.heading_weight,
            Weight::Explicit(weight) => weight,
        };
        let text: SharedString = if self.uppercase {
            self.text.to_uppercase().into()
        } else {
            self.text
        };
        // The interface font, with its fallbacks, is inherited from the window root; only the
        // monospaced face has to be named here.
        let mut element = div();
        if self.mono {
            element = element.font(theme.typography.mono.clone());
        }
        let mut element = element
            .text_size(self.size.pixels(theme))
            .text_color(self.color.resolve(theme))
            .font_weight(weight)
            .child(text);
        if self.truncate {
            element = element.truncate();
        }
        element
    }
}
