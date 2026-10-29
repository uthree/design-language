//! How much of the person's plan is left, shown where they spend it.

use gpui::{
    AnyView, App, AppContext, ClickEvent, Context, ElementId, InteractiveElement, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};

use crate::display::Tone;
use crate::focus::focusable;
use crate::progress::{ProgressBar, ProgressRing};
use crate::strings::{ActiveStrings, Strings};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One limit the plan sets: a rolling window, a week, a monthly budget.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageLimit {
    label: SharedString,
    used: f32,
    resets: Option<SharedString>,
}

impl UsageLimit {
    /// A limit named `label` ("5-hour window", "Weekly") with `used` of it spent, from 0 to 1.
    pub fn new(label: impl Into<SharedString>, used: f32) -> Self {
        Self {
            label: label.into(),
            used: used.clamp(0., 1.),
            resets: None,
        }
    }

    /// When it fills up again, already formatted: "in 2 h 10 min", "Mon 09:00".
    pub fn resets(mut self, resets: impl Into<SharedString>) -> Self {
        self.resets = Some(resets.into());
        self
    }

    /// The part still left, from 0 to 1.
    pub fn remaining(&self) -> f32 {
        1. - self.used
    }
}

/// The plan's remaining allowance, for the composer's control row: a small ring that drains as
/// the allowance is spent, with the percentage left beside it.
///
/// Give it every [`UsageLimit`] the plan has; the ring follows the one closest to running out,
/// and hovering lists them all with when each resets. Quiet while there is room, it turns to
/// the warning colour below [`UsageMeter::warn_below`] and to danger below
/// [`UsageMeter::danger_below`]; once a limit is spent it says so and when it comes back,
/// instead of a percentage. Pass it to [`super::Composer::usage`].
#[derive(IntoElement)]
pub struct UsageMeter {
    id: ElementId,
    limits: Vec<UsageLimit>,
    warn_below: f32,
    danger_below: f32,
    on_click: Option<ClickHandler>,
}

impl UsageMeter {
    /// A meter with no limits yet.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            limits: Vec::new(),
            warn_below: 0.2,
            danger_below: 0.05,
            on_click: None,
        }
    }

    /// A limit of the plan. Add them in the order the plan names them; the hover lists them so.
    pub fn limit(mut self, limit: UsageLimit) -> Self {
        self.limits.push(limit);
        self
    }

    /// The share left, from 0 to 1, under which the meter turns to the warning colour. 0.2 by
    /// default.
    pub fn warn_below(mut self, remaining: f32) -> Self {
        self.warn_below = remaining;
        self
    }

    /// The share left under which the meter turns to danger. 0.05 by default.
    pub fn danger_below(mut self, remaining: f32) -> Self {
        self.danger_below = remaining;
        self
    }

    /// Called on press; open the plan's usage page there. Makes the meter a tab stop.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// The limit closest to running out.
    fn tightest(&self) -> Option<&UsageLimit> {
        self.limits.iter().max_by(|a, b| a.used.total_cmp(&b.used))
    }

    fn tone(&self, remaining: f32) -> Tone {
        tone_for(remaining, self.warn_below, self.danger_below)
    }
}

fn tone_for(remaining: f32, warn_below: f32, danger_below: f32) -> Tone {
    if remaining <= danger_below {
        Tone::Danger
    } else if remaining < warn_below {
        Tone::Warning
    } else {
        Tone::Neutral
    }
}

fn percent(fraction: f32) -> String {
    format!("{}", (fraction * 100.).round() as u32)
}

impl RenderOnce for UsageMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let strings = cx.strings().clone();
        let Some(tightest) = self.tightest().cloned() else {
            return div().into_any_element();
        };
        let remaining = tightest.remaining();
        let tone = self.tone(remaining);
        let spent = remaining <= 0.;
        let text_color = match tone {
            Tone::Neutral => colors.text_faint,
            tone => tone.colors(theme).0,
        };
        let height = theme.density.control_sm;
        let radius = theme.control_radius(height);
        let hover = colors.hover;
        let label: SharedString = if spent {
            match &tightest.resets {
                Some(resets) => strings.usage_resets(resets),
                None => strings.usage_reached.clone(),
            }
        } else {
            strings.usage_left(&percent(remaining))
        };
        let details = UsageDetails {
            limits: self.limits.clone(),
            warn_below: self.warn_below,
            danger_below: self.danger_below,
            strings: strings.clone(),
        };

        let clickable = self.on_click.is_some();
        let mut meter = focusable(
            div().id(self.id.clone()),
            &self.id,
            !clickable,
            radius,
            window,
            cx,
        )
        .flex()
        .flex_none()
        .items_center()
        .gap(theme.spacing.xs)
        .h(height)
        .px(theme.spacing.xs)
        .rounded(radius)
        .text_size(theme.typography.size_xs)
        .text_color(text_color)
        .when(tone != Tone::Neutral, |meter| {
            meter.bg(tone.colors(theme).1)
        })
        .when(clickable, |meter| {
            meter.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .child(
            ProgressRing::new(remaining)
                .id((self.id.clone(), "ring"))
                .size(px(14.))
                .tone(tone),
        )
        .child(div().whitespace_nowrap().child(label))
        .tooltip(move |_window, cx| -> AnyView { cx.new(|_| details.clone()).into() });
        if let Some(handler) = self.on_click {
            meter = meter.on_click(handler);
        }
        meter.into_any_element()
    }
}

/// The hover over a [`UsageMeter`]: every limit, how much of it is used and when it resets.
#[derive(Clone)]
struct UsageDetails {
    limits: Vec<UsageLimit>,
    warn_below: f32,
    danger_below: f32,
    strings: Strings,
}

impl Render for UsageDetails {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let strings = &self.strings;
        div()
            .font(theme.typography.ui.clone())
            .flex()
            .flex_col()
            .gap(theme.spacing.sm)
            .w(px(240.))
            .px(theme.spacing.md)
            .py(theme.spacing.sm)
            .rounded(theme.shape.radius_sm)
            .bg(theme.colors.overlay)
            .border_1()
            .border_color(theme.colors.border)
            .shadow(theme.elevation.medium.clone())
            .text_size(theme.typography.size_sm)
            .text_color(theme.colors.text)
            .child(
                Label::new(strings.usage.clone())
                    .size(TextSize::XSmall)
                    .color(TextColor::Muted),
            )
            .children(self.limits.iter().map(|limit| {
                let tone = tone_for(limit.remaining(), self.warn_below, self.danger_below);
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xxs)
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(theme.spacing.sm)
                            .child(div().flex_1().truncate().child(limit.label.clone()))
                            .child(
                                Label::new(strings.usage_used(&percent(limit.used)))
                                    .size(TextSize::XSmall)
                                    .color(match tone {
                                        Tone::Danger => TextColor::Danger,
                                        Tone::Warning => TextColor::Warning,
                                        _ => TextColor::Muted,
                                    }),
                            ),
                    )
                    .child(
                        ProgressBar::new(limit.used)
                            .height(px(4.))
                            .tone(match tone {
                                Tone::Neutral => Tone::Accent,
                                tone => tone,
                            }),
                    )
                    .children(limit.resets.as_ref().map(|resets| {
                        Label::new(strings.usage_resets(resets))
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint)
                    }))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_tightest_limit() {
        let meter = UsageMeter::new("usage")
            .limit(UsageLimit::new("5-hour", 0.3))
            .limit(UsageLimit::new("Weekly", 0.85));
        assert_eq!(meter.tightest().map(|l| l.label.as_ref()), Some("Weekly"));
    }

    #[test]
    fn warns_then_alarms_as_it_runs_out() {
        assert_eq!(tone_for(0.5, 0.2, 0.05), Tone::Neutral);
        assert_eq!(tone_for(0.15, 0.2, 0.05), Tone::Warning);
        assert_eq!(tone_for(0.02, 0.2, 0.05), Tone::Danger);
        assert_eq!(tone_for(0., 0.2, 0.05), Tone::Danger);
    }
}
