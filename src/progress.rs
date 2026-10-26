//! Waiting and progress: bars, rings, skeletons, long-running tasks and multi-step plans.
//!
//! Pick by what is known. If the fraction done is known, show it ([`ProgressBar::new`],
//! [`ProgressRing`]). If only "working" is known, say so without inventing a number
//! ([`ProgressBar::indeterminate`], [`crate::Spinner`]). If the shape of the coming content is
//! known, sketch it ([`Skeleton`]). Show none of these for work that finishes before the eye can
//! read them.

use std::f32::consts::PI;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Div, ElementId, Hsla, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StyleRefinement, Styled, Window, canvas, div, linear_color_stop,
    linear_gradient, prelude::FluentBuilder, prelude::Refineable, px, relative,
};

use crate::button::{Button, ControlSize};
use crate::controls::arc;
use crate::display::{Spinner, Tone};
use crate::icon::{Icon, IconName};
use crate::motion::{ease_out, lerp, looping, transition, tween};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::{ActiveTheme, Theme};

fn tone_fill(tone: Tone, theme: &Theme) -> Hsla {
    match tone {
        Tone::Accent => theme.colors.accent,
        other => other.colors(theme).0,
    }
}

/// A horizontal progress bar, determinate or indeterminate.
#[derive(IntoElement)]
pub struct ProgressBar {
    id: Option<ElementId>,
    value: Option<f32>,
    tone: Tone,
    height: Option<Pixels>,
}

impl ProgressBar {
    /// A bar at `value`, from 0 to 1.
    pub fn new(value: f32) -> Self {
        Self {
            id: None,
            value: Some(value.clamp(0., 1.)),
            tone: Tone::Accent,
            height: None,
        }
    }

    /// A bar for work whose length is unknown: a segment sweeps across the track.
    pub fn indeterminate(id: impl Into<ElementId>) -> Self {
        Self {
            id: Some(id.into()),
            value: None,
            tone: Tone::Accent,
            height: None,
        }
    }

    /// Change the value, keeping the other settings.
    pub fn value(mut self, value: f32) -> Self {
        self.value = Some(value.clamp(0., 1.));
        self
    }

    /// A stable id. With one, the fill glides to each new value instead of jumping.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Fill colour by meaning: [`Tone::Agent`] for work the agent is doing.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Track thickness; defaults to the spacing scale's `xs`.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let height = self.height.unwrap_or(theme.spacing.xs);
        let fill = tone_fill(self.tone, &theme);
        let segment = || {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .rounded(height / 2.)
                .bg(fill)
        };
        let fill_element = match (self.value, self.id) {
            (Some(value), Some(id)) => tween(
                segment().left_0().w(relative(value)),
                id,
                value,
                theme.motion.normal,
                window,
                cx,
                |fill, value| fill.w(relative(value)),
            ),
            (Some(value), None) => segment().left_0().w(relative(value)).into_any_element(),
            (None, id) => looping(
                segment().w(relative(0.3)),
                id.unwrap_or_else(|| "progress-indeterminate".into()),
                theme.motion.slow,
                0.35,
                cx,
                |segment, delta| {
                    let eased = gpui::ease_in_out(delta);
                    segment.left(relative(lerp(-0.3, 1., eased)))
                },
            ),
        };
        div()
            .relative()
            .w_full()
            .h(height)
            .rounded(height / 2.)
            .bg(theme.colors.sunken)
            .overflow_hidden()
            .child(fill_element)
    }
}

/// A circular progress mark with the percentage in the middle, for compact places: a list row,
/// a clip header, a toolbar.
#[derive(IntoElement)]
pub struct ProgressRing {
    id: Option<ElementId>,
    value: f32,
    size: Pixels,
    tone: Tone,
    show_value: bool,
}

impl ProgressRing {
    /// A ring at `value`, from 0 to 1.
    pub fn new(value: f32) -> Self {
        Self {
            id: None,
            value: value.clamp(0., 1.),
            size: px(28.),
            tone: Tone::Accent,
            show_value: false,
        }
    }

    /// A stable id, so the arc glides to new values.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Diameter.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Fill colour by meaning.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Print the percentage inside the ring. Needs a diameter of about 32 px or more.
    pub fn show_value(mut self) -> Self {
        self.show_value = true;
        self
    }

    /// Change the value, keeping the other settings.
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0., 1.);
        self
    }
}

/// The painted part of a [`ProgressRing`], separate so a tween can redraw it at any value.
#[derive(IntoElement)]
struct Ring {
    value: f32,
    size: Pixels,
    track: Hsla,
    fill: Hsla,
}

impl Ring {
    fn value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }
}

impl RenderOnce for Ring {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Self {
            value,
            size,
            track,
            fill,
        } = self;
        canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window, _| {
                let center = bounds.center();
                let width = (size * 0.1).max(px(2.));
                let radius = size / 2. - width;
                let start = -PI / 2.;
                if let Some(path) = arc(center, radius, start, start + 2. * PI, width) {
                    window.paint_path(path, track);
                }
                if let Some(path) = arc(center, radius, start, start + 2. * PI * value, width) {
                    window.paint_path(path, fill);
                }
            },
        )
        .size(size)
    }
}

impl RenderOnce for ProgressRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let ring = Ring {
            value: self.value,
            size: self.size,
            track: theme.colors.sunken,
            fill: tone_fill(self.tone, &theme),
        };
        let ring = match self.id {
            Some(id) => tween(
                ring,
                id,
                self.value,
                theme.motion.normal,
                window,
                cx,
                Ring::value,
            ),
            None => ring.into_any_element(),
        };
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .child(div().absolute().inset_0().child(ring))
            .when(self.show_value, |ring| {
                ring.child(
                    div()
                        .font(theme.typography.mono.clone())
                        .text_size(self.size * 0.28)
                        .text_color(theme.colors.text_muted)
                        .child(format!("{}", (self.value * 100.).round() as u32)),
                )
            })
    }
}

/// A placeholder in the shape of content that is still loading. A highlight sweeps across it.
///
/// Use it only when the shape is known — a list of rows, a waveform lane — and only while
/// the content is really loading.
#[derive(IntoElement)]
pub struct Skeleton {
    id: ElementId,
    base: Div,
}

impl Skeleton {
    /// A placeholder block; size it with `.w(..)` and `.h(..)`. Defaults to one line of text.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            base: div(),
        }
    }
}

impl Styled for Skeleton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let block = theme.colors.hover;
        let shine = if theme.appearance.is_dark() {
            gpui::white().opacity(0.06)
        } else {
            gpui::white().opacity(0.7)
        };
        let band = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w(relative(0.5))
            .bg(linear_gradient(
                90.,
                linear_color_stop(shine.opacity(0.), 0.),
                linear_color_stop(shine, 1.),
            ));
        // Defaults first, then whatever size and radius the caller gave.
        let mut caller = self.base;
        let caller_style = caller.style().clone();
        let mut placeholder = div()
            .h(theme.typography.size_md)
            .rounded(theme.shape.radius_xs);
        placeholder.style().refine(&caller_style);
        placeholder
            .relative()
            .overflow_hidden()
            .bg(block)
            .child(looping(
                band,
                self.id,
                theme.motion.slow,
                -1.,
                cx,
                |band, delta| band.left(relative(lerp(-0.5, 1., delta))),
            ))
    }
}

/// Lines of text that are still loading: full-width lines and a shorter last one.
pub fn skeleton_paragraph(id: impl Into<SharedString>, lines: usize, cx: &App) -> Div {
    let id = id.into();
    let gap = cx.theme().spacing.sm;
    div()
        .flex()
        .flex_col()
        .gap(gap)
        .children((0..lines).map(|line| {
            let width = if line + 1 == lines { 0.6 } else { 1. };
            Skeleton::new(ElementId::NamedInteger(id.clone(), line as u64)).w(relative(width))
        }))
}

/// Where a long-running task is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// Waiting for another task to finish.
    Queued,
    /// Running.
    Running,
    /// Finished.
    Succeeded,
    /// Stopped by an error.
    Failed,
    /// Stopped by the person.
    Cancelled,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A long-running job — a render, an export, a model download, an agent's batch edit — with its
/// progress, what it is doing now, and a way to stop it.
#[derive(IntoElement)]
pub struct TaskProgress {
    id: ElementId,
    title: SharedString,
    detail: Option<SharedString>,
    icon: IconName,
    state: TaskState,
    progress: Option<f32>,
    remaining: Option<SharedString>,
    tone: Tone,
    on_cancel: Option<ClickHandler>,
    action: Option<AnyElement>,
}

impl TaskProgress {
    /// A running task with unknown progress.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            icon: IconName::Waveform,
            state: TaskState::Running,
            progress: None,
            remaining: None,
            tone: Tone::Accent,
            on_cancel: None,
            action: None,
        }
    }

    /// What it is doing now, for example "Bouncing stem 3 of 8".
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The task's icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// Where it is.
    pub fn state(mut self, state: TaskState) -> Self {
        self.state = state;
        self
    }

    /// Fraction done, when known.
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0., 1.));
        self
    }

    /// Time left, already phrased: "About 20 s left".
    pub fn remaining(mut self, remaining: impl Into<SharedString>) -> Self {
        self.remaining = Some(remaining.into());
        self
    }

    /// Mark it as the agent's work.
    pub fn by_agent(mut self) -> Self {
        self.tone = Tone::Agent;
        self
    }

    /// Show Cancel while it runs and call this on it.
    pub fn on_cancel(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_cancel = Some(Box::new(handler));
        self
    }

    /// An action once it has stopped: "Show in folder", "Retry".
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for TaskProgress {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let state = self.state;
        let live = matches!(state, TaskState::Running | TaskState::Queued);
        let (status, status_color) = match state {
            TaskState::Queued => ("Queued".into(), colors.text_faint),
            TaskState::Running => match self.progress {
                Some(progress) => (
                    SharedString::from(format!("{} %", (progress * 100.).round() as u32)),
                    colors.text_muted,
                ),
                None => ("Working".into(), colors.text_muted),
            },
            TaskState::Succeeded => ("Done".into(), colors.success),
            TaskState::Failed => ("Failed".into(), colors.danger),
            TaskState::Cancelled => ("Cancelled".into(), colors.text_faint),
        };
        let mark = match state {
            TaskState::Running => Spinner::new((self.id.clone(), "spinner"))
                .size(px(14.))
                .color(tone_fill(self.tone, &theme))
                .into_any_element(),
            TaskState::Succeeded => Icon::new(IconName::Check)
                .size(px(14.))
                .color(colors.success)
                .into_any_element(),
            TaskState::Failed => Icon::new(IconName::Alert)
                .size(px(14.))
                .color(colors.danger)
                .into_any_element(),
            TaskState::Queued | TaskState::Cancelled => Icon::new(self.icon)
                .size(px(14.))
                .color(colors.text_faint)
                .into_any_element(),
        };
        let mark = transition(
            div()
                .flex()
                .flex_none()
                .w(px(16.))
                .justify_center()
                .child(mark),
            (self.id.clone(), "mark"),
            &state,
            theme.motion.normal,
            window,
            cx,
            |mark, delta| mark.opacity(delta),
        );
        let bar = live.then(|| match (state, self.progress) {
            (TaskState::Running, Some(progress)) => ProgressBar::new(progress)
                .id((self.id.clone(), "bar"))
                .tone(self.tone),
            (TaskState::Running, None) => {
                ProgressBar::indeterminate((self.id.clone(), "bar")).tone(self.tone)
            }
            _ => ProgressBar::new(0.).tone(Tone::Neutral),
        });
        let cancel = self.on_cancel.filter(|_| live).map(|handler| {
            Button::new((self.id.clone(), "cancel"), "Cancel")
                .ghost()
                .size(ControlSize::Small)
                .on_click(handler)
        });

        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.sm)
            .p(theme.spacing.md)
            .rounded(theme.shape.radius_md)
            .bg(colors.elevated)
            .border_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(mark)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                Label::new(self.title)
                                    .weight(theme.typography.label_weight)
                                    .truncate(),
                            )
                            .children(self.detail.map(|detail| {
                                Label::new(detail)
                                    .size(TextSize::Small)
                                    .color(TextColor::Muted)
                                    .truncate()
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_end()
                            .child(
                                Label::new(status)
                                    .size(TextSize::Small)
                                    .mono()
                                    .color(TextColor::Custom(status_color)),
                            )
                            .children(self.remaining.filter(|_| live).map(|remaining| {
                                Label::new(remaining)
                                    .size(TextSize::XSmall)
                                    .color(TextColor::Faint)
                            })),
                    )
                    .children(cancel)
                    .children(self.action.filter(|_| !live)),
            )
            .children(bar)
    }
}

/// Where one step of a plan is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    /// Not started.
    Pending,
    /// In progress.
    Running,
    /// Finished.
    Done,
    /// Stopped by an error.
    Failed,
    /// Passed over.
    Skipped,
}

struct Step {
    label: SharedString,
    detail: Option<SharedString>,
    state: StepState,
}

/// A vertical list of steps joined by a line: an agent's plan, an export pipeline, a setup
/// checklist. The running step is the only one drawn at full strength.
#[derive(IntoElement)]
pub struct StepList {
    id: ElementId,
    steps: Vec<Step>,
}

impl StepList {
    /// An empty list.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            steps: Vec::new(),
        }
    }

    /// Add a step.
    pub fn step(mut self, label: impl Into<SharedString>, state: StepState) -> Self {
        self.steps.push(Step {
            label: label.into(),
            detail: None,
            state,
        });
        self
    }

    /// Add a step with a muted line under it, for example what the running step is doing.
    pub fn step_with_detail(
        mut self,
        label: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        state: StepState,
    ) -> Self {
        self.steps.push(Step {
            label: label.into(),
            detail: Some(detail.into()),
            state,
        });
        self
    }
}

impl RenderOnce for StepList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = &theme.colors;
        let count = self.steps.len();
        let marker = px(16.);
        let rows: Vec<AnyElement> = self
            .steps
            .into_iter()
            .enumerate()
            .map(|(index, step)| {
                let id = ElementId::NamedChild(Box::new(self.id.clone()), index.to_string().into());
                let dot = match step.state {
                    StepState::Pending | StepState::Skipped => div()
                        .size(marker - px(4.))
                        .rounded(marker)
                        .border_1()
                        .border_color(colors.border_strong)
                        .into_any_element(),
                    StepState::Running => Spinner::new((id.clone(), "spinner"))
                        .size(marker)
                        .into_any_element(),
                    StepState::Done => div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(marker)
                        .rounded(marker)
                        .bg(colors.success)
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(10.))
                                .color(theme.text_on(colors.success)),
                        )
                        .into_any_element(),
                    StepState::Failed => div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(marker)
                        .rounded(marker)
                        .bg(colors.danger)
                        .child(
                            Icon::new(IconName::Close)
                                .size(px(10.))
                                .color(theme.text_on(colors.danger)),
                        )
                        .into_any_element(),
                };
                // A step that changes state pops its new marker in.
                let dot = transition(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(marker)
                        .child(dot),
                    (id.clone(), "marker"),
                    &step.state,
                    theme.motion.normal,
                    window,
                    cx,
                    move |dot, delta| {
                        dot.opacity(delta)
                            .size(marker * lerp(0.6, 1., ease_out(delta)))
                    },
                );
                let label_color = match step.state {
                    StepState::Running => TextColor::Default,
                    StepState::Done => TextColor::Muted,
                    StepState::Failed => TextColor::Danger,
                    StepState::Pending | StepState::Skipped => TextColor::Faint,
                };
                let last = index + 1 == count;
                let line = if step.state == StepState::Done {
                    colors.success.opacity(0.5)
                } else {
                    colors.border
                };
                div()
                    .flex()
                    .gap(theme.spacing.sm)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .w(marker)
                            .child(dot)
                            .when(!last, |lane| {
                                lane.child(
                                    div().w(px(1.5)).flex_1().min_h(theme.spacing.md).bg(line),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .pb(if last { px(0.) } else { theme.spacing.md })
                            .child({
                                let label = Label::new(step.label).color(label_color);
                                if step.state == StepState::Running {
                                    label.weight(theme.typography.label_weight)
                                } else {
                                    label
                                }
                            })
                            .children(step.detail.map(|detail| {
                                Label::new(detail)
                                    .size(TextSize::Small)
                                    .color(TextColor::Faint)
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        div().flex().flex_col().children(rows)
    }
}
