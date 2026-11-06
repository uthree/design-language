//! Asking before acting: the approval dialog and the permission mode the person picks.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Corner, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, prelude::FluentBuilder, px,
};

use super::chat::Decision;
use super::code::CodeBlock;
use crate::button::{Button, ButtonVariant, ControlSize};
use crate::controls::held;
use crate::dialog::Dialog;
use crate::display::{Spinner, Tone};
use crate::focus::{focusable, own_handle};
use crate::icon::{Icon, IconName};
use crate::overlay::{MenuItem, Popover};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

type DecisionHandler = Rc<dyn Fn(&Decision, &mut Window, &mut App) + 'static>;

/// An approval as a modal dialog, for a request that must stop everything: the agent is about
/// to run a command or change files and the person is not looking at the conversation, or the
/// application has no conversation on screen.
///
/// Deny takes focus when it appears, so Enter refuses, and Escape or a press outside denies
/// too. The allowing button names the verb ("Run", "Replace") and turns to danger for an
/// action that destroys work. While an automatic reviewer looks at the request
/// ([`ApprovalDialog::reviewing`]), the dialog says so and only Deny can be pressed.
///
/// Inside a conversation, put the same request in the transcript instead, with
/// [`super::ChatEntry::approval`]; it answers with the same [`Decision`].
#[derive(IntoElement)]
pub struct ApprovalDialog {
    id: ElementId,
    title: SharedString,
    body: Option<SharedString>,
    command: Option<SharedString>,
    detail: Option<AnyElement>,
    allow_label: Option<SharedString>,
    destructive: bool,
    offer_always: bool,
    reviewing: bool,
    on_decide: Option<DecisionHandler>,
}

impl ApprovalDialog {
    /// A request titled with its verb and object: `Run "cargo test --workspace"?`.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: None,
            command: None,
            detail: None,
            allow_label: None,
            destructive: false,
            offer_always: false,
            reviewing: false,
            on_decide: None,
        }
    }

    /// Scope and consequence: what it touches, whether it can be undone.
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// The exact command it would run, shown as a terminal line.
    pub fn command(mut self, command: impl Into<SharedString>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Anything else to judge it by, such as a [`super::DiffBlock`] of the change.
    pub fn detail(mut self, detail: impl IntoElement) -> Self {
        self.detail = Some(detail.into_any_element());
        self
    }

    /// The allowing button's verb; [`crate::Strings::allow`] by default.
    pub fn allow_label(mut self, label: impl Into<SharedString>) -> Self {
        self.allow_label = Some(label.into());
        self
    }

    /// The action destroys work: the allowing button is drawn as danger.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Offer "Always allow" on the leading end.
    pub fn offer_always(mut self) -> Self {
        self.offer_always = true;
        self
    }

    /// An automatic reviewer is deciding: say so, and leave only Deny pressable.
    pub fn reviewing(mut self, reviewing: bool) -> Self {
        self.reviewing = reviewing;
        self
    }

    /// Called with the person's answer. Escape and a press outside answer
    /// [`Decision::Deny`].
    pub fn on_decide(
        mut self,
        handler: impl Fn(&Decision, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ApprovalDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let strings = cx.strings().clone();
        let deny_focus = own_handle(&(self.id.clone(), "deny").into(), window, cx);
        let decide = self.on_decide;
        let answer = move |decision: Decision| {
            let decide = decide.clone();
            move |window: &mut Window, cx: &mut App| {
                if let Some(decide) = &decide {
                    decide(&decision, window, cx);
                }
            }
        };
        let click = |decision: Decision| {
            let answer = answer(decision);
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| answer(window, cx)
        };
        let theme = cx.theme();
        let tone = if self.destructive {
            Tone::Danger
        } else {
            Tone::Warning
        };
        let mut dialog = Dialog::new(self.id.clone(), self.title)
            .icon(IconName::Alert, tone)
            .initial_focus(&deny_focus)
            .on_dismiss({
                let deny = answer(Decision::Deny);
                move |_: &(), window: &mut Window, cx: &mut App| deny(window, cx)
            });
        if let Some(body) = self.body {
            dialog = dialog.body(body);
        }
        if let Some(command) = self.command {
            dialog = dialog.child(CodeBlock::new(
                (self.id.clone(), "command"),
                format!("$ {command}"),
            ));
        }
        if let Some(detail) = self.detail {
            dialog = dialog.child(detail);
        }
        if self.reviewing {
            dialog = dialog.child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .child(Spinner::new((self.id.clone(), "reviewing")).size(px(13.)))
                    .child(
                        Label::new(strings.reviewing.clone())
                            .size(TextSize::Small)
                            .color(TextColor::Muted),
                    ),
            );
        }
        if self.offer_always {
            dialog = dialog.leading_action(
                Button::new((self.id.clone(), "always"), strings.always_allow.clone())
                    .ghost()
                    .disabled(self.reviewing)
                    .on_click(click(Decision::AlwaysAllow)),
            );
        }
        dialog
            .action(
                Button::new((self.id.clone(), "deny"), strings.deny.clone())
                    .track_focus(&deny_focus)
                    .on_click(click(Decision::Deny)),
            )
            .action(
                Button::new(
                    (self.id.clone(), "allow"),
                    self.allow_label.unwrap_or(strings.allow),
                )
                .variant(if self.destructive {
                    ButtonVariant::Danger
                } else {
                    ButtonVariant::Primary
                })
                .disabled(self.reviewing)
                .on_click(click(Decision::Allow)),
            )
    }
}

/// How much the agent may do before asking. Applications offer the ones their runtime
/// supports, in this order, from the most careful to the least.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PermissionMode {
    /// Read and propose; change nothing.
    Plan,
    /// Ask before every edit and command.
    #[default]
    Ask,
    /// Edit files without asking; ask before commands.
    AcceptEdits,
    /// A reviewer model answers each request; uncertain ones come to the person.
    AutoReview,
    /// Run everything without asking. Says so for as long as it is chosen.
    Bypass,
}

impl PermissionMode {
    /// Every mode, from the most careful to the least.
    pub const ALL: [PermissionMode; 5] = [
        Self::Plan,
        Self::Ask,
        Self::AcceptEdits,
        Self::AutoReview,
        Self::Bypass,
    ];

    /// Its name, from [`crate::Strings::permission_modes`].
    pub fn label(self, cx: &App) -> SharedString {
        cx.strings().permission_modes[self as usize].clone()
    }

    /// What it lets the agent do, from [`crate::Strings::permission_details`].
    pub fn detail(self, cx: &App) -> SharedString {
        cx.strings().permission_details[self as usize].clone()
    }

    /// Its icon.
    pub fn icon(self) -> IconName {
        match self {
            Self::Plan => IconName::Eye,
            Self::Ask => IconName::User,
            Self::AcceptEdits => IconName::File,
            Self::AutoReview => IconName::Sparkles,
            Self::Bypass => IconName::Alert,
        }
    }

    /// Whether it takes the person out of the loop, and so has to stay visible in the
    /// danger colour for as long as it is chosen rather than once when it is picked.
    pub fn warns(self) -> bool {
        self == Self::Bypass
    }
}

type ModeHandler = Rc<dyn Fn(&PermissionMode, &mut Window, &mut App) + 'static>;

/// The permission mode as a small button for the composer row, opening a menu of the modes
/// with what each one lets the agent do.
///
/// A mode that takes the person out of the loop ([`PermissionMode::warns`]) keeps the button in
/// the danger colour while chosen. Without [`PermissionPicker::on_select`] it keeps the chosen
/// mode itself.
#[derive(IntoElement)]
pub struct PermissionPicker {
    id: ElementId,
    mode: PermissionMode,
    modes: Vec<PermissionMode>,
    on_select: Option<ModeHandler>,
}

impl PermissionPicker {
    /// A picker showing `mode`.
    pub fn new(id: impl Into<ElementId>, mode: PermissionMode) -> Self {
        Self {
            id: id.into(),
            mode,
            modes: PermissionMode::ALL.to_vec(),
            on_select: None,
        }
    }

    /// The modes on offer, in the order to list them; all five by default.
    pub fn modes(mut self, modes: impl IntoIterator<Item = PermissionMode>) -> Self {
        self.modes = modes.into_iter().collect();
        self
    }

    /// Called with the mode picked.
    pub fn on_select(
        mut self,
        handler: impl Fn(&PermissionMode, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PermissionPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (mode, set_mode) = held(&self.id, self.mode, window, cx);
        let (open, set_open) = held(
            &ElementId::NamedChild(Box::new(self.id.clone()), "open".into()),
            false,
            window,
            cx,
        );
        let select: ModeHandler = match self.on_select {
            Some(handler) => handler,
            None => Rc::new(
                move |mode: &PermissionMode, window: &mut Window, cx: &mut App| {
                    set_mode(mode, window, cx)
                },
            ),
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let warns = mode.warns();
        let text = if warns {
            colors.danger
        } else {
            colors.text_muted
        };
        let hover = if warns {
            colors.danger_wash
        } else {
            colors.hover
        };
        let trigger = focusable(
            div().id(self.id.clone()),
            &self.id,
            false,
            theme.shape.radius_sm,
            window,
            cx,
        )
        .flex()
        .items_center()
        .gap(theme.spacing.xs)
        .h(ControlSize::Small.height(theme))
        .px(theme.spacing.sm)
        .rounded(theme.shape.radius_sm)
        .text_size(theme.typography.size_sm)
        .text_color(text)
        .when(warns, |trigger| trigger.bg(colors.danger_wash))
        .hover(move |style| style.bg(hover))
        .child(Icon::new(mode.icon()).size(px(13.)).color(text))
        .child(div().whitespace_nowrap().child(mode.label(cx)))
        .child(Icon::new(IconName::ChevronDown).size(px(11.)).color(text))
        .on_click({
            let set_open = set_open.clone();
            move |_, window, cx| set_open(&!open, window, cx)
        });

        let menu = open.then(|| {
            let close = set_open.clone();
            let close_out = close.clone();
            let mut popover = Popover::new()
                .id((self.id.clone(), "menu"))
                .w(px(300.))
                .child(
                    div().px(px(8.)).pt(px(4.)).pb(px(2.)).child(
                        Label::new(cx.strings().permissions.clone())
                            .size(TextSize::XSmall)
                            .color(TextColor::Faint),
                    ),
                );
            for (index, option) in self.modes.iter().copied().enumerate() {
                let select = select.clone();
                let close = close.clone();
                let mut item = MenuItem::new(
                    ElementId::NamedChild(
                        Box::new(self.id.clone()),
                        format!("mode-{index}").into(),
                    ),
                    option.label(cx),
                )
                .icon(option.icon())
                .description(option.detail(cx))
                .checked(option == mode)
                .on_click(move |_, window, cx| {
                    close(&false, window, cx);
                    select(&option, window, cx);
                });
                if option.warns() {
                    item = item.danger();
                }
                popover = popover.child(item);
            }
            deferred(
                anchored().anchor(Corner::BottomLeft).child(
                    div()
                        .mb(theme.spacing.xs)
                        .occlude()
                        .on_mouse_down_out(move |_, window, cx| close_out(&false, window, cx))
                        .child(popover),
                ),
            )
            .with_priority(1)
        });
        div()
            .relative()
            .flex_none()
            .when(open, |picker| {
                picker.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        set_open(&false, window, cx);
                    }
                })
            })
            .children(menu)
            .child(trigger)
    }
}
