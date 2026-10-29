//! Forms and preferences: labelled fields, groups of them, and the status line under a page.
//!
//! A preferences window is a [`crate::Sidebar`] of pages beside one page at a time. Each page
//! opens with a [`crate::SectionHeader`] (title and one line of description), holds
//! [`FormGroup`]s of [`FormField`]s in a column no wider than `density.form_width`, and ends in
//! its actions, the primary at the trailing end. A [`StatusBar`] along the window's bottom edge
//! reports what the last action did. The `preferences` example builds one.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::display::Tone;
use crate::icon::{Icon, IconName};
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// Where a field's label goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelPlacement {
    /// Above the control, aligned to its leading edge. The default, and the rule for anything
    /// typed into: text fields, pickers, sliders. Long and translated labels never squeeze the
    /// control.
    #[default]
    Above,
    /// At the leading edge of the row, with the control at the trailing edge. For a control
    /// that is a yes or no — a switch, a checkbox — so the label reads as the question and the
    /// control as the answer.
    Beside,
}

/// One labelled setting: the label, an optional description, the control, and an error when
/// the value is wrong.
///
/// ```ignore
/// FormField::new("Model folder")
///     .description("Where downloaded voices are kept.")
///     .child(TextField::new().value("~/Auris/Voices"))
/// FormField::new("Fast mode").beside().child(Switch::new("fast", on))
/// ```
///
/// With the label above, the first control fills the column's width — a field, a picker, a
/// list — and any after it (a "Browse…" button) keep their own width beside it.
#[derive(IntoElement)]
pub struct FormField {
    label: Option<SharedString>,
    description: Option<SharedString>,
    note: Option<SharedString>,
    error: Option<SharedString>,
    placement: LabelPlacement,
    control: Vec<AnyElement>,
}

impl FormField {
    /// A field labelled `label`, with its label above the control.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            description: None,
            note: None,
            error: None,
            placement: LabelPlacement::Above,
            control: Vec::new(),
        }
    }

    /// A field without a label of its own, when the [`FormGroup`] title already names it: a
    /// group holding one setting. Never use it where the group holds several.
    pub fn unlabelled() -> Self {
        Self {
            label: None,
            ..Self::new("")
        }
    }

    /// A second, conditional line under the description: a caution that applies only to the
    /// current choice ("Clicking now creates notes on the grid"). Leave it out otherwise rather
    /// than leaving an empty line.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// One sentence on what the setting does, in muted text under the label. Say what changes,
    /// not what the control is.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// What is wrong with the value and how to fix it. Drawn under the control in the danger
    /// colour with a mark, never by colour alone.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Where the label goes.
    pub fn placement(mut self, placement: LabelPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Shorthand for [`LabelPlacement::Beside`], for switches and checkboxes.
    pub fn beside(self) -> Self {
        self.placement(LabelPlacement::Beside)
    }
}

impl ParentElement for FormField {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.control.extend(elements)
    }
}

impl RenderOnce for FormField {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let label = self.label.map(|label| {
            div()
                .text_size(theme.typography.size_sm)
                .font_weight(theme.typography.label_weight)
                .text_color(theme.colors.text)
                .child(label)
        });
        let note = self.note.map(|note| {
            Label::new(note)
                .size(TextSize::Small)
                .color(TextColor::Muted)
        });
        let description = self.description.map(|description| {
            Label::new(description)
                .size(TextSize::Small)
                .color(TextColor::Muted)
        });
        let error = self.error.map(|error| {
            div()
                .flex()
                .items_center()
                .gap(theme.spacing.xs)
                .text_size(theme.typography.size_sm)
                .text_color(theme.colors.danger)
                .child(
                    Icon::new(IconName::Alert)
                        .size(px(13.))
                        .color(theme.colors.danger),
                )
                .child(error)
        });
        match self.placement {
            LabelPlacement::Above => {
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.xs)
                    .children(label)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .w_full()
                            .gap(theme.spacing.sm)
                            .children(self.control.into_iter().enumerate().map(
                                |(index, control)| {
                                    // The first control stretches across; the rest keep their width.
                                    if index == 0 {
                                        div().flex().flex_col().flex_1().min_w_0().child(control)
                                    } else {
                                        div().flex().flex_none().child(control)
                                    }
                                },
                            )),
                    )
                    .children(description)
                    .children(note)
                    .children(error)
            }
            LabelPlacement::Beside => div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xs)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(theme.spacing.lg)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(theme.spacing.xxs)
                                .children(label)
                                .children(description)
                                .children(note),
                        )
                        .child(div().flex().flex_none().children(self.control)),
                )
                .children(error),
        }
    }
}

/// A titled group of [`FormField`]s on a preferences page or in a dialog.
#[derive(IntoElement)]
pub struct FormGroup {
    title: Option<SharedString>,
    description: Option<SharedString>,
    fields: Vec<AnyElement>,
}

impl FormGroup {
    /// A group titled `title`.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: Some(title.into()),
            description: None,
            fields: Vec::new(),
        }
    }

    /// A group with no title, for a page with only one.
    pub fn untitled() -> Self {
        Self {
            title: None,
            description: None,
            fields: Vec::new(),
        }
    }

    /// One line under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

impl ParentElement for FormGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.fields.extend(elements)
    }
}

impl RenderOnce for FormGroup {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let heading = self.title.map(|title| {
            div()
                .flex()
                .flex_col()
                .gap(theme.spacing.xxs)
                .child(Label::new(title).size(TextSize::Medium).heading_weight())
                .children(self.description.map(|description| {
                    Label::new(description)
                        .size(TextSize::Small)
                        .color(TextColor::Muted)
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap(theme.spacing.md)
            .children(heading)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(theme.spacing.lg)
                    .p(theme.density.panel_padding)
                    .rounded(theme.shape.radius_md)
                    .bg(colors.elevated)
                    .border_1()
                    .border_color(colors.border)
                    .children(self.fields),
            )
    }
}

/// The thin line along a window's bottom edge that says what just happened: "Saved", "Model
/// downloaded", "Could not reach the server". It reports; it never asks. Something the person
/// must answer goes in a dialog or next to its cause.
///
/// A message in a tone other than neutral carries its mark, so the state is not colour alone.
/// Put a [`crate::Spinner`] first with [`StatusBar::busy`] while the work it reports on runs.
#[derive(IntoElement)]
pub struct StatusBar {
    message: Option<SharedString>,
    tone: Tone,
    busy: bool,
    leading: Vec<AnyElement>,
    trailing: Vec<AnyElement>,
}

impl StatusBar {
    /// An empty status bar.
    pub fn new() -> Self {
        Self {
            message: None,
            tone: Tone::Neutral,
            busy: false,
            leading: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// What happened, in a few words.
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// What the message means: success, a warning, a failure, or the agent's doing.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The work the message is about is still running: a spinner goes before it.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Content before the message.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading.push(element.into_any_element());
        self
    }

    /// Content at the trailing end: a position, a count, a sample rate, a small control.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
        self
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for StatusBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let (color, _) = self.tone.colors(theme);
        let mark = match self.tone {
            Tone::Neutral | Tone::Accent => None,
            Tone::Success => Some(IconName::Check),
            Tone::Warning | Tone::Danger => Some(IconName::Alert),
            Tone::Agent => Some(IconName::Sparkles),
        };
        let has_message = self.message.is_some();
        let text = match self.tone {
            Tone::Neutral => colors.text_muted,
            _ => color,
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.spacing.sm)
            .w_full()
            .h(theme.density.status_bar)
            .px(theme.spacing.md)
            .bg(colors.chrome)
            .border_t_1()
            .border_color(colors.border)
            .text_size(theme.typography.size_xs)
            .text_color(colors.text_muted)
            .when(self.busy, |bar| {
                bar.child(
                    crate::display::Spinner::new("atelier-status-busy")
                        .size(px(12.))
                        .color(color),
                )
            })
            .children(self.leading)
            .children(
                mark.filter(|_| !self.busy)
                    .map(|mark| Icon::new(mark).size(px(12.)).color(color)),
            )
            .children(self.message.map(|message| {
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(text)
                    .child(message)
            }))
            .when(!has_message, |bar| bar.child(div().flex_1()))
            .children(self.trailing)
    }
}
