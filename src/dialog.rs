//! The modal dialog: one question the person answers before going on.

use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Pixels, RenderOnce, SharedString, Styled, Window, anchored, deferred, div,
    point, prelude::FluentBuilder, px,
};

use crate::display::Tone;
use crate::focus::{own_handle, selector};
use crate::icon::{Icon, IconName};
use crate::motion::appear;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

type DismissHandler = Rc<dyn Fn(&(), &mut Window, &mut App) + 'static>;

/// The key context a dialog's keys run in.
pub const DIALOG_CONTEXT: &str = "AtelierDialog";

/// A modal dialog: a card over a scrim that dims the whole window, so the question it asks is
/// the one thing to answer.
///
/// Draw it while it is open, from anywhere in the window: it lays itself over everything. It
/// takes focus when it appears — the control given to [`Dialog::initial_focus`], usually the
/// safe choice, or else the dialog itself — and Escape or a press on the scrim calls
/// [`Dialog::on_dismiss`]. Put the focus back where it was when you close it.
///
/// Keep the title a question or a verb and its object ("Delete “Verse 2”?"), the body to what
/// the title cannot say, and the actions to two or three, the safe one first.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    icon: Option<(IconName, Tone)>,
    body: Option<SharedString>,
    content: Vec<AnyElement>,
    actions: Vec<AnyElement>,
    leading: Vec<AnyElement>,
    width: Pixels,
    initial_focus: Option<FocusHandle>,
    on_dismiss: Option<DismissHandler>,
}

impl Dialog {
    /// A dialog asking `title`.
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            body: None,
            content: Vec::new(),
            actions: Vec::new(),
            leading: Vec::new(),
            width: px(440.),
            initial_focus: None,
            on_dismiss: None,
        }
    }

    /// An icon before the title, in a tone's colour: [`Tone::Warning`] for an approval,
    /// [`Tone::Danger`] for a loss.
    pub fn icon(mut self, icon: IconName, tone: Tone) -> Self {
        self.icon = Some((icon, tone));
        self
    }

    /// What the title cannot say: scope, and whether it can be undone.
    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Anything else under the body: a command, a diff, a form.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }

    /// A button on the trailing end of the footer. Add the safe choice first and the one the
    /// dialog exists for last, so the last sits in the corner.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.actions.push(action.into_any_element());
        self
    }

    /// A quieter control on the leading end of the footer, such as "Always allow".
    pub fn leading_action(mut self, action: impl IntoElement) -> Self {
        self.leading.push(action.into_any_element());
        self
    }

    /// The card's width; 440 px by default. It never grows past the window.
    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    /// Focus this control when the dialog appears, usually the safe choice, so Enter answers
    /// the way that loses nothing.
    pub fn initial_focus(mut self, handle: &FocusHandle) -> Self {
        self.initial_focus = Some(handle.clone());
        self
    }

    /// Called on Escape and on a press outside the card. Without it, the dialog can only be
    /// answered with its actions. It takes `&()` so that `cx.listener(|this, _: &(), window, cx|
    /// ..)` fits.
    pub fn on_dismiss(mut self, handler: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let own = own_handle(&self.id, window, cx);
        // Once per appearance: the flag lives in element state, which gpui drops in the first
        // frame the dialog is not drawn, so the next opening takes focus again.
        let first = window.with_global_id(
            ElementId::NamedChild(Box::new(self.id.clone()), "opened".into()),
            |global_id, window| {
                window.with_element_state(global_id, |done: Option<bool>, _| (done.is_none(), true))
            },
        );
        if first {
            // After the frame's own start-up focus, which the window root defers the same way.
            let handle = self.initial_focus.clone().unwrap_or_else(|| own.clone());
            window.defer(cx, move |window, _| window.focus(&handle));
        }

        let theme = cx.theme();
        let colors = &theme.colors;
        let viewport = window.viewport_size();
        let title = div()
            .flex()
            .items_center()
            .gap(theme.spacing.sm)
            .children(
                self.icon
                    .map(|(icon, tone)| Icon::new(icon).size(px(16.)).color(tone.colors(theme).0)),
            )
            .child(
                Label::new(self.title)
                    .size(TextSize::Large)
                    .weight(theme.typography.heading_weight),
            );
        let footer = (!self.actions.is_empty() || !self.leading.is_empty()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(theme.spacing.sm)
                .pt(theme.spacing.xs)
                .children(self.leading)
                .child(div().flex_1())
                .children(self.actions)
        });
        let card = div()
            .id((self.id.clone(), "card"))
            .track_focus(&own)
            .key_context(DIALOG_CONTEXT)
            .flex()
            .flex_col()
            .gap(theme.spacing.md)
            .w(self.width)
            .max_w(viewport.width - theme.spacing.xl * 2.)
            .max_h(viewport.height - theme.spacing.xl * 2.)
            .p(theme.spacing.lg)
            .rounded(theme.shape.radius_lg)
            .bg(colors.overlay)
            .border_1()
            .border_color(colors.border)
            .shadow(theme.elevation.high.clone())
            .text_color(colors.text)
            // A press on the card stays on the card.
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(title)
            .children(self.body.map(|body| {
                Label::new(body)
                    .size(TextSize::Small)
                    .color(TextColor::Muted)
            }))
            .children(self.content)
            .children(footer);
        let card = appear(card, (self.id.clone(), "appear"), cx, |card, delta| {
            card.opacity(delta)
                .mt(px(crate::motion::lerp(8., 0., delta)))
        });

        let dismiss = self.on_dismiss;
        let scrim = selector(div().id(self.id.clone()), &self.id)
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .w(viewport.width)
            .h(viewport.height)
            .bg(colors.scrim)
            .when_some(dismiss.clone(), |scrim, dismiss| {
                scrim.on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                    dismiss(&(), window, cx)
                })
            })
            .when_some(dismiss, |scrim, dismiss| {
                scrim.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        dismiss(&(), window, cx);
                    }
                })
            })
            .child(card);
        deferred(anchored().position(point(px(0.), px(0.))).child(scrim)).with_priority(2)
    }
}
