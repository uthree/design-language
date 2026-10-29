//! Read-only marks: badges, keyboard hints, avatars, progress, meters and activity indicators.

use std::time::Duration;

use gpui::{
    App, Bounds, ElementId, Hsla, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
    Styled, Transformation, Window, canvas, div, fill, linear_color_stop, linear_gradient,
    percentage, point, prelude::FluentBuilder, px, size, svg,
};

use crate::icon::{Icon, IconName, IconSource};
use crate::motion::looping;
use crate::theme::{ActiveTheme, Theme};

/// The meaning of a badge or notice. Keep most badges [`Tone::Neutral`]: a row of coloured
/// badges is usually a missing hierarchy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// No particular meaning.
    #[default]
    Neutral,
    /// The accent.
    Accent,
    /// Something succeeded.
    Success,
    /// Something deserves attention.
    Warning,
    /// Something failed.
    Danger,
    /// The AI did this.
    Agent,
}

impl Tone {
    /// Foreground and wash colours under `theme`.
    pub fn colors(self, theme: &Theme) -> (Hsla, Hsla) {
        let colors = &theme.colors;
        match self {
            Self::Neutral => (colors.text_muted, colors.hover),
            Self::Accent => (colors.accent_text, colors.accent_wash),
            Self::Success => (colors.success, colors.success_wash),
            Self::Warning => (colors.warning, colors.warning_wash),
            Self::Danger => (colors.danger, colors.danger_wash),
            Self::Agent => (colors.agent, colors.agent_wash),
        }
    }
}

/// A short state, count or classification.
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    tone: Tone,
    icon: Option<IconName>,
    dot: bool,
}

impl Badge {
    /// A neutral badge.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            tone: Tone::Neutral,
            icon: None,
            dot: false,
        }
    }

    /// What the badge means.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Leading icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Leading status dot.
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (foreground, wash) = self.tone.colors(theme);
        let height = theme.typography.size_xs + theme.spacing.sm;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.spacing.xs)
            .h(height)
            .px(theme.spacing.sm - theme.spacing.xxs)
            .rounded(if theme.shape.capsule_controls {
                height / 2.
            } else {
                theme.shape.radius_xs
            })
            .bg(wash)
            .text_size(theme.typography.size_xs)
            .font_weight(theme.typography.label_weight)
            .text_color(foreground)
            .children(self.dot.then(|| StatusDot::new(foreground).size(px(6.))))
            .children(self.icon.map(|icon| {
                Icon::new(icon)
                    .size(theme.typography.size_xs)
                    .color(foreground)
            }))
            .child(self.label)
    }
}

/// A keyboard key, for shortcut hints.
#[derive(IntoElement)]
pub struct Kbd {
    keys: SharedString,
}

impl Kbd {
    /// Keys already formatted for the platform, for example "⌘K" or "Ctrl+K".
    pub fn new(keys: impl Into<SharedString>) -> Self {
        Self { keys: keys.into() }
    }
}

impl RenderOnce for Kbd {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(theme.typography.size_xs + theme.spacing.sm)
            .px(theme.spacing.xs)
            .rounded(theme.shape.radius_xs)
            .border_1()
            .border_color(theme.colors.border_strong)
            .bg(theme.colors.elevated)
            .font(theme.typography.mono.clone())
            .text_size(theme.typography.size_xs)
            .text_color(theme.colors.text_muted)
            .child(self.keys)
    }
}

/// Who an avatar stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum AvatarKind {
    /// A person, shown by initials.
    Person(SharedString),
    /// The AI agent, shown on its gradient.
    Agent,
    /// The AI agent, shown by the mark of the provider whose model is answering — usually a
    /// [`crate::ProviderLogo`]. Prefer it to [`AvatarKind::Agent`] wherever the model is known:
    /// the mark says whose model is talking, which the gradient cannot.
    Logo(IconSource),
}

/// A round identity mark.
#[derive(IntoElement)]
pub struct Avatar {
    kind: AvatarKind,
    size: Pixels,
}

impl Avatar {
    /// A person's avatar from their initials.
    pub fn person(initials: impl Into<SharedString>) -> Self {
        Self {
            kind: AvatarKind::Person(initials.into()),
            size: px(24.),
        }
    }

    /// The agent's avatar.
    pub fn agent() -> Self {
        Self {
            kind: AvatarKind::Agent,
            size: px(24.),
        }
    }

    /// The agent shown by its provider's mark, drawn bare in the text colour.
    pub fn logo(logo: impl Into<IconSource>) -> Self {
        Self {
            kind: AvatarKind::Logo(logo.into()),
            size: px(24.),
        }
    }

    /// Diameter.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let base = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded(self.size / 2.);
        match self.kind {
            AvatarKind::Person(initials) => base
                .bg(theme.colors.elevated)
                .border_1()
                .border_color(theme.colors.border_strong)
                .text_size(self.size * 0.42)
                .font_weight(theme.typography.label_weight)
                .text_color(theme.colors.text_muted)
                .child(initials),
            AvatarKind::Agent => {
                let [from, _, to] = theme.colors.agent_gradient;
                base.bg(linear_gradient(
                    135.,
                    linear_color_stop(from, 0.),
                    linear_color_stop(to, 1.),
                ))
                .child(
                    Icon::new(IconName::Sparkles)
                        .size(self.size * 0.56)
                        .color(gpui::white()),
                )
            }
            // No disc: a provider's mark is its own shape, and a ring around it reads as a
            // second logo.
            AvatarKind::Logo(logo) => base.child(
                Icon::new(logo)
                    .size(self.size * 0.86)
                    .color(theme.colors.text),
            ),
        }
    }
}

/// A one-pixel rule. Draw it on the boundary's owner only, never on both neighbours.
#[derive(IntoElement)]
pub struct Divider {
    vertical: bool,
    full: bool,
}

impl Divider {
    /// A horizontal rule.
    pub fn horizontal() -> Self {
        Self {
            vertical: false,
            full: false,
        }
    }

    /// A vertical rule, for toolbars.
    pub fn vertical() -> Self {
        Self {
            vertical: true,
            full: false,
        }
    }

    /// Span the parent instead of one control height, for separating whole regions.
    pub fn full(mut self) -> Self {
        self.full = true;
        self
    }
}

impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let color = theme.colors.separator;
        if self.vertical {
            div()
                .flex_none()
                .w(px(1.))
                .when_else(
                    self.full,
                    |rule| rule.h_full(),
                    |rule| rule.h(theme.density.control_sm),
                )
                .bg(color)
        } else {
            div().flex_none().h(px(1.)).w_full().bg(color)
        }
    }
}

/// A level meter: green to yellow to red, with a peak hold line.
#[derive(IntoElement)]
pub struct LevelMeter {
    level: f32,
    peak: Option<f32>,
    vertical: bool,
    thickness: Pixels,
}

impl LevelMeter {
    /// A meter showing `level`, from 0 (silence) to 1 (full scale), on the display's own curve.
    pub fn new(level: f32) -> Self {
        Self {
            level: level.clamp(0., 1.),
            peak: None,
            vertical: true,
            thickness: px(6.),
        }
    }

    /// Change the level, keeping the other settings.
    pub fn level(mut self, level: f32) -> Self {
        self.level = level.clamp(0., 1.);
        self
    }

    /// Peak hold position.
    pub fn peak(mut self, peak: f32) -> Self {
        self.peak = Some(peak.clamp(0., 1.));
        self
    }

    /// Lie horizontally.
    pub fn horizontal(mut self) -> Self {
        self.vertical = false;
        self
    }

    /// Width of a vertical meter, height of a horizontal one.
    pub fn thickness(mut self, thickness: Pixels) -> Self {
        self.thickness = thickness;
        self
    }
}

/// Where the meter turns from safe to hot, and from hot to clipping.
const METER_ZONES: [f32; 2] = [0.7, 0.9];

impl RenderOnce for LevelMeter {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let track = theme.colors.sunken;
        let colors = theme.colors.meter;
        let peak_color = theme.colors.text;
        let (level, peak, vertical) = (self.level, self.peak, self.vertical);
        let element = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                window.paint_quad(fill(bounds, track).corner_radii(px(2.)));
                let length = if vertical {
                    bounds.size.height
                } else {
                    bounds.size.width
                };
                // Each zone keeps its own colour up to the level, so colour marks the zone.
                let span = |from: f32, to: f32| -> Bounds<Pixels> {
                    if vertical {
                        Bounds::new(
                            point(bounds.left(), bounds.bottom() - length * to),
                            size(bounds.size.width, length * (to - from)),
                        )
                    } else {
                        Bounds::new(
                            point(bounds.left() + length * from, bounds.top()),
                            size(length * (to - from), bounds.size.height),
                        )
                    }
                };
                let edges = [0., METER_ZONES[0], METER_ZONES[1], 1.];
                for (zone, color) in colors.iter().enumerate() {
                    let (from, to) = (edges[zone], edges[zone + 1].min(level));
                    if to > from {
                        window.paint_quad(fill(span(from, to), *color));
                    }
                }
                if let Some(peak) = peak {
                    let at = (peak - 0.012).max(0.);
                    window.paint_quad(fill(span(at, at + 0.012), peak_color));
                }
            },
        );
        if self.vertical {
            element.w(self.thickness).h_full()
        } else {
            element.h(self.thickness).w_full()
        }
    }
}

/// An indeterminate activity indicator. Show it only while something is really waiting.
#[derive(IntoElement)]
pub struct Spinner {
    id: ElementId,
    size: Pixels,
    color: Option<Hsla>,
}

impl Spinner {
    /// A spinner; `id` keeps its animation stable across frames.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            size: px(14.),
            color: None,
        }
    }

    /// Edge length.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Colour; defaults to the agent colour, since most waiting is waiting for the model.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl RenderOnce for Spinner {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let spinner = svg()
            .path(IconName::Spinner.path())
            .size(self.size)
            .flex_none()
            .text_color(self.color.unwrap_or(cx.theme().colors.agent));
        looping(
            spinner,
            self.id,
            Duration::from_millis(900),
            0.,
            cx,
            |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
        )
    }
}

/// A small coloured dot. Pair it with a word or a mark: colour alone is not a state.
#[derive(IntoElement)]
pub struct StatusDot {
    color: Hsla,
    size: Pixels,
}

impl StatusDot {
    /// A dot in `color`.
    pub fn new(color: Hsla) -> Self {
        Self {
            color,
            size: px(8.),
        }
    }

    /// Diameter.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for StatusDot {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex_none()
            .size(self.size)
            .rounded(self.size / 2.)
            .bg(self.color)
    }
}
