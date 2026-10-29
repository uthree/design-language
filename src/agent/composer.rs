//! Where the person writes to the agent, and the chips around it.

use gpui::{
    AnyElement, AnyView, App, ClickEvent, ElementId, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::display::{Kbd, Tone};
use crate::focus::focusable;
use crate::icon::{Icon, IconName, IconSource};
use crate::overlay::Tooltip;
use crate::strings::ActiveStrings;
use crate::theme::{ActiveTheme, Theme};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The message box: context chips, the text, and a row of controls ending in Send.
///
/// The text area is a slot: pass the application's editable input with [`Composer::input`] and
/// say whether it is [`Composer::empty`], or a static [`Composer::value`] in mock-ups. The row
/// under it holds attach, the model picker and any [`Composer::control`]s — the reasoning
/// effort, a fast-mode switch — before Send. While `busy`, Send becomes Stop.
#[derive(IntoElement)]
pub struct Composer {
    id: ElementId,
    value: Option<SharedString>,
    input: Option<AnyElement>,
    empty: Option<bool>,
    placeholder: Option<SharedString>,
    context: Vec<AnyElement>,
    controls: Vec<AnyElement>,
    completions: Option<AnyElement>,
    model: Option<SharedString>,
    attach_disabled: bool,
    tone: Option<Tone>,
    notice: Option<AnyElement>,
    busy: bool,
    focused: bool,
    on_send: Option<ClickHandler>,
    on_stop: Option<ClickHandler>,
    on_attach: Option<ClickHandler>,
    on_model: Option<ClickHandler>,
}

impl Composer {
    /// An empty composer.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            input: None,
            empty: None,
            placeholder: None,
            context: Vec::new(),
            controls: Vec::new(),
            completions: None,
            model: None,
            attach_disabled: false,
            tone: None,
            notice: None,
            busy: false,
            focused: false,
            on_send: None,
            on_stop: None,
            on_attach: None,
            on_model: None,
        }
    }

    /// Static text, for mock-ups.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into()).filter(|value: &SharedString| !value.is_empty());
        self
    }

    /// The application's editable input element.
    pub fn input(mut self, input: impl IntoElement) -> Self {
        self.input = Some(input.into_any_element());
        self
    }

    /// Whether the application's input holds no text. The placeholder shows and Send is
    /// disabled while it does; without it an input counts as never empty.
    pub fn empty(mut self, empty: bool) -> Self {
        self.empty = Some(empty);
        self
    }

    /// Text shown while empty.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// A control in the row under the text, after the model picker: the reasoning effort, a
    /// fast-mode switch, a voice picker. Use small ghost buttons and small segmented controls,
    /// so the row stays one line and Send stays the only filled control in it.
    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.controls.push(control.into_any_element());
        self
    }

    /// Suggestions for what is being typed, usually [`crate::Completions`] for slash commands
    /// or model names. They float over the composer, clear of the transcript's last line, and
    /// leave focus in the input.
    pub fn completions(mut self, completions: impl IntoElement) -> Self {
        self.completions = Some(completions.into_any_element());
        self
    }

    /// Grey out the attach button, for example while a take is being generated.
    pub fn attach_disabled(mut self, disabled: bool) -> Self {
        self.attach_disabled = disabled;
        self
    }

    /// Draw the frame in a signal colour while a mode needs remembering — approvals skipped,
    /// a destructive tool allowed. A mark joins the colour in the control row; say what the
    /// mode is in a [`Composer::control`] or [`Composer::notice`] beside it.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// A line inside the frame under the text, before the control row: a connection error
    /// with its Retry, a note that context was compacted. Usually a
    /// [`super::ChatNotice`].
    pub fn notice(mut self, notice: impl IntoElement) -> Self {
        self.notice = Some(notice.into_any_element());
        self
    }

    /// Context that will be sent with the message, usually [`ContextChip`]s.
    pub fn context(mut self, chip: impl IntoElement) -> Self {
        self.context.push(chip.into_any_element());
        self
    }

    /// The model picker's label.
    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// A turn is running: show Stop instead of Send.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Draw the focus ring.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Called on Send.
    pub fn on_send(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Box::new(handler));
        self
    }

    /// Called on Stop.
    pub fn on_stop(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_stop = Some(Box::new(handler));
        self
    }

    /// Called on the attach button.
    pub fn on_attach(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_attach = Some(Box::new(handler));
        self
    }

    /// Called on the model picker; open the model menu there.
    pub fn on_model(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_model = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Composer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let outer = theme.shape.radius_lg;
        let inset = theme.spacing.sm;
        // Controls in the bottom row share the composer's corner centre.
        let control = theme.density.control_md;
        let control_radius = Theme::concentric(outer, inset).min(control / 2.);
        let empty = match self.empty {
            Some(empty) => empty && self.value.is_none(),
            None => self.value.is_none() && self.input.is_none(),
        };
        let has_text = !empty;

        let strings = cx.strings().clone();
        let mut attach = Button::icon_only((self.id.clone(), "attach"), IconName::Paperclip)
            .disabled(self.attach_disabled)
            .tooltip(strings.attach.clone());
        if let Some(handler) = self.on_attach {
            attach = attach.on_click(handler);
        }
        let model = self.model.map(|model| {
            let mut button = Button::new((self.id.clone(), "model"), model)
                .ghost()
                .size(ControlSize::Small)
                .icon_after(IconName::ChevronDown);
            if let Some(handler) = self.on_model {
                button = button.on_click(handler);
            }
            button
        });
        let action = if self.busy {
            let mut stop = Button::icon_only((self.id.clone(), "stop"), IconName::Stop)
                .variant(ButtonVariant::Secondary)
                .radius(control_radius)
                .tooltip(strings.stop.clone());
            if let Some(handler) = self.on_stop {
                stop = stop.on_click(handler);
            }
            stop
        } else {
            let mut send = Button::icon_only((self.id.clone(), "send"), IconName::ArrowUp)
                .primary()
                .disabled(!has_text)
                .radius(control_radius)
                .tooltip(strings.send.clone());
            if let Some(handler) = self.on_send {
                send = send.on_click(handler);
            }
            send
        };

        div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .gap(theme.spacing.xs)
            .p(inset)
            .rounded(outer)
            .bg(colors.elevated)
            .border_1()
            .border_color(match self.tone {
                Some(tone) if tone != Tone::Neutral => tone.colors(theme).0,
                _ if self.focused => colors.accent.opacity(0.7),
                _ => colors.border_strong,
            })
            .shadow(theme.elevation.low.clone())
            .when(!self.context.is_empty(), |composer| {
                composer.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(theme.spacing.xs)
                        .children(self.context),
                )
            })
            .child(
                div()
                    .min_h(theme.typography.size_md * 3.)
                    .px(theme.spacing.xs)
                    .py(theme.spacing.xs)
                    .text_size(theme.typography.size_md)
                    .children(
                        self.value
                            .map(|value| div().text_color(colors.text).child(value)),
                    )
                    // The placeholder sits under the application's input, where its caret is.
                    .relative()
                    .when(empty, |area| {
                        area.child(
                            div()
                                .absolute()
                                .top(theme.spacing.xs)
                                .left(theme.spacing.xs)
                                .text_color(colors.text_faint)
                                .child(
                                    self.placeholder
                                        .unwrap_or_else(|| strings.ask_anything.clone()),
                                ),
                        )
                    })
                    .children(self.input),
            )
            .children(self.notice)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.xs)
                    // The tone's mark, so the frame's colour is never the only sign.
                    .children(self.tone.filter(|tone| *tone != Tone::Neutral).map(|tone| {
                        div().pl(theme.spacing.xs).child(
                            Icon::new(match tone {
                                Tone::Agent => IconName::Sparkles,
                                Tone::Success => IconName::Check,
                                _ => IconName::Alert,
                            })
                            .size(px(14.))
                            .color(tone.colors(theme).0),
                        )
                    }))
                    .child(attach)
                    .children(model)
                    .children(self.controls)
                    .child(div().flex_1())
                    .when(!self.busy, |row| {
                        row.child(Kbd::new(if cfg!(target_os = "macos") {
                            "↩"
                        } else {
                            "Enter"
                        }))
                    })
                    // The send button's radius follows the composer's corner, not the capsule rule.
                    .child(action),
            )
            .children(self.completions.map(|completions| {
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_full()
                    .mb(theme.spacing.xs)
                    .child(completions)
            }))
    }
}

/// A piece of context attached to a message: a track, a clip, a file, a selection.
#[derive(IntoElement)]
pub struct ContextChip {
    id: ElementId,
    label: SharedString,
    icon: IconSource,
    color: Option<gpui::Hsla>,
    remove_label: Option<SharedString>,
    on_click: Option<ClickHandler>,
    on_remove: Option<ClickHandler>,
}

impl ContextChip {
    /// A chip.
    pub fn new(
        id: impl Into<ElementId>,
        icon: impl Into<IconSource>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            color: None,
            remove_label: None,
            on_click: None,
            on_remove: None,
        }
    }

    /// Called when the chip itself is pressed — to open a preview of the attachment. Makes its
    /// icon and label a tab stop of their own, apart from the remove button.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// The remove button's tooltip, naming what goes; [`crate::Strings::remove`] with the
    /// label by default ("Remove transport.rs", "transport.rsを外す").
    pub fn remove_label(mut self, label: impl Into<SharedString>) -> Self {
        self.remove_label = Some(label.into());
        self
    }

    /// Icon colour, for example the track's colour.
    pub fn color(mut self, color: gpui::Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// Show a remove button and call this on it.
    pub fn on_remove(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ContextChip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let height = theme.density.control_sm;
        let hover = colors.hover;
        let remove_label = self
            .remove_label
            .unwrap_or_else(|| cx.strings().remove(&self.label));
        let remove = self.on_remove.map(|handler| {
            let id = ElementId::from((self.id.clone(), "remove"));
            focusable(
                div().id(id.clone()),
                &id,
                false,
                theme.shape.radius_xs,
                window,
                cx,
            )
            .flex()
            .items_center()
            .justify_center()
            .size(height - px(6.))
            .rounded(theme.shape.radius_xs)
            .hover(move |style| style.bg(hover))
            .child(Icon::new(IconName::Close).size(px(10.)))
            .tooltip(move |window, cx| -> AnyView {
                Tooltip::new(remove_label.clone()).build(window, cx)
            })
            .on_click(handler)
        });
        let content = div()
            .flex()
            .items_center()
            .gap(theme.spacing.xs)
            .child(
                Icon::new(self.icon)
                    .size(px(12.))
                    .color(self.color.unwrap_or(colors.text_faint)),
            )
            .child(self.label);
        // Pressing the chip and removing it are separate stops, so neither press reaches the
        // other.
        let content = match self.on_click {
            Some(handler) => {
                let id = ElementId::from((self.id.clone(), "open"));
                focusable(
                    div().id(id.clone()),
                    &id,
                    false,
                    theme.shape.radius_xs,
                    window,
                    cx,
                )
                .rounded(theme.shape.radius_xs)
                .cursor_pointer()
                .hover(|style| style.text_color(colors.text))
                .child(content)
                .on_click(handler)
                .into_any_element()
            }
            None => content.into_any_element(),
        };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.spacing.xs)
            .h(height)
            .pl(theme.spacing.sm - theme.spacing.xxs)
            .pr(if remove.is_some() {
                theme.spacing.xxs
            } else {
                theme.spacing.sm
            })
            .rounded(theme.control_radius(height))
            .bg(colors.sunken)
            .border_1()
            .border_color(colors.border)
            .text_size(theme.typography.size_xs)
            .text_color(colors.text_muted)
            .child(content)
            .children(remove)
    }
}

/// A suggested prompt. Clicking it should fill or send the composer.
#[derive(IntoElement)]
pub struct SuggestionChip {
    id: ElementId,
    label: SharedString,
    icon: Option<IconSource>,
    on_click: Option<ClickHandler>,
}

impl SuggestionChip {
    /// A suggestion.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            on_click: None,
        }
    }

    /// Leading icon.
    pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Called on click.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for SuggestionChip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let hover = colors.hover;
        let mut chip = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_click.is_none(),
            theme.shape.radius_md,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .px(theme.spacing.md)
        .py(theme.spacing.sm)
        .rounded(theme.shape.radius_md)
        .border_1()
        .border_color(colors.border)
        .text_size(theme.typography.size_sm)
        .text_color(colors.text_muted)
        .hover(move |style| style.bg(hover))
        .children(
            self.icon
                .map(|icon| Icon::new(icon).size(px(14.)).color(colors.agent)),
        )
        .child(div().flex_1().min_w_0().child(self.label));
        if let Some(handler) = self.on_click {
            chip = chip.on_click(handler);
        }
        chip
    }
}

/// A one-line entry point to the agent, placed where the work is: over a selection, at the top
/// of an inspector. It looks like a field and opens the panel or an inline prompt.
#[derive(IntoElement)]
pub struct AskBar {
    id: ElementId,
    placeholder: SharedString,
    shortcut: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl AskBar {
    /// An entry point.
    pub fn new(id: impl Into<ElementId>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            placeholder: placeholder.into(),
            shortcut: None,
            on_click: None,
        }
    }

    /// The shortcut that opens the same prompt.
    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Called on click.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AskBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let height = theme.density.control_md;
        let hover = colors.agent.opacity(0.5);
        let mut bar = focusable(
            div().id(self.id.clone()),
            &self.id,
            self.on_click.is_none(),
            theme.control_radius(height),
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.sm)
        .h(height)
        .px(theme.spacing.md)
        .rounded(theme.control_radius(height))
        .bg(colors.agent_wash)
        .border_1()
        .border_color(colors.agent.opacity(0.25))
        .hover(move |style| style.border_color(hover))
        .child(
            Icon::new(IconName::Sparkles)
                .size(px(14.))
                .color(colors.agent),
        )
        .child(
            div()
                .flex_1()
                .truncate()
                .text_size(theme.typography.size_sm)
                .text_color(colors.text_muted)
                .child(self.placeholder),
        )
        .children(self.shortcut.map(Kbd::new));
        if let Some(handler) = self.on_click {
            bar = bar.on_click(handler);
        }
        bar
    }
}
