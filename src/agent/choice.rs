//! A question the agent asked with fixed answers.

use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce, SharedString,
    Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::button::{Button, ButtonVariant, ControlSize};
use crate::controls::{Checkbox, held};
use crate::icon::IconName;
use crate::layout::{Card, ListItem};
use crate::strings::ActiveStrings;
use crate::text::{Label, TextColor, TextSize};
use crate::theme::ActiveTheme;

/// One answer on a [`ChoiceCard`]: a label and, under it, what picking it means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// The answer, as the person would say it.
    pub label: SharedString,
    /// A line on what it leads to.
    pub detail: Option<SharedString>,
}

impl Choice {
    /// An answer.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            detail: None,
        }
    }

    /// A line under the label on what picking it leads to.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl From<&'static str> for Choice {
    fn from(label: &'static str) -> Self {
        Self::new(label)
    }
}

impl From<String> for Choice {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

impl From<SharedString> for Choice {
    fn from(label: SharedString) -> Self {
        Self::new(label)
    }
}

type ChooseHandler = Rc<dyn Fn(&Vec<usize>, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// A question the agent asked with fixed answers, waiting on the person.
///
/// It is the turn's whole point, not a tool row, so it stands as a card. With one answer to
/// give, each option is a [`ListItem`] that answers on the first press, the way a person
/// answers out loud; [`ChoiceCard::multiple`] gives each a [`Checkbox`] and a Send button.
/// Once [`ChoiceCard::answered`], the card stays readable with what was picked and takes no
/// more input. In a [`super::ChatView`], push [`super::ChatEntry::choices`] instead and listen
/// for [`super::ChatEvent::Chose`].
///
/// Options are `{id}-option-{index}`; the Send button is `{id}-send`.
///
/// ```ignore
/// ChoiceCard::new("tempo", "Which tempo should the chorus use?")
///     .option(Choice::new("Keep 120 BPM"))
///     .option(Choice::new("Double time").detail("240 BPM, drums only"))
///     .on_choose(|picked, _, _| send_answer(picked))
/// ```
#[derive(IntoElement)]
pub struct ChoiceCard {
    id: ElementId,
    question: SharedString,
    options: Vec<Choice>,
    multiple: bool,
    picked: Option<Vec<usize>>,
    answered: bool,
    on_choose: Option<ChooseHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl ChoiceCard {
    /// A question.
    pub fn new(id: impl Into<ElementId>, question: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            question: question.into(),
            options: Vec::new(),
            multiple: false,
            picked: None,
            answered: false,
            on_choose: None,
            on_toggle: None,
        }
    }

    /// Add an answer.
    pub fn option(mut self, choice: impl Into<Choice>) -> Self {
        self.options.push(choice.into());
        self
    }

    /// Add answers.
    pub fn options<C: Into<Choice>>(mut self, choices: impl IntoIterator<Item = C>) -> Self {
        self.options.extend(choices.into_iter().map(Into::into));
        self
    }

    /// Let the person pick several, then send them together.
    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self
    }

    /// The ticked answers of a [`ChoiceCard::multiple`] card, if the application keeps them;
    /// otherwise the card keeps them itself.
    pub fn picked(mut self, picked: Vec<usize>) -> Self {
        self.picked = Some(picked);
        self
    }

    /// The card was answered with `picked`: show that, and take no more input.
    pub fn answered(mut self, picked: Vec<usize>) -> Self {
        self.picked = Some(picked);
        self.answered = true;
        self
    }

    /// Called with the picked answers' indices: one, on a press, or several, on Send.
    pub fn on_choose(
        mut self,
        handler: impl Fn(&Vec<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_choose = Some(Rc::new(handler));
        self
    }

    /// Called when an answer of a [`ChoiceCard::multiple`] card is ticked or unticked, for an
    /// application that keeps [`ChoiceCard::picked`].
    pub fn on_toggle(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    fn child(&self, part: impl Into<SharedString>) -> ElementId {
        ElementId::NamedChild(Box::new(self.id.clone()), part.into())
    }
}

impl RenderOnce for ChoiceCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active = !self.answered;
        let held_id = self.child("picks");
        let (picked, set_picked) = match (&self.picked, &self.on_toggle) {
            (Some(picked), Some(_)) => (picked.clone(), None),
            _ => {
                let given = self.picked.clone().unwrap_or_default();
                let (picked, set) = held(&held_id, given, window, cx);
                (picked, Some(set))
            }
        };
        let theme = cx.theme();
        let strings = cx.strings();
        let options: Vec<_> = self
            .options
            .iter()
            .enumerate()
            .map(|(index, option)| {
                let chosen = picked.contains(&index);
                let id = self.child(format!("option-{index}"));
                if self.multiple {
                    let mut checkbox = Checkbox::new(id, chosen)
                        .label(option.label.clone())
                        .disabled(!active);
                    if active {
                        let on_toggle = self.on_toggle.clone();
                        let set = set_picked.clone();
                        let picked = picked.clone();
                        checkbox = checkbox.on_click(move |_, window, cx| {
                            if let Some(on_toggle) = &on_toggle {
                                on_toggle(&index, window, cx);
                            }
                            if let Some(set) = &set {
                                let mut next = picked.clone();
                                match next.iter().position(|&ticked| ticked == index) {
                                    Some(at) => {
                                        next.remove(at);
                                    }
                                    None => next.push(index),
                                }
                                set(&next, window, cx);
                            }
                        });
                    }
                    div()
                        .flex()
                        .flex_col()
                        .gap(theme.spacing.xxs)
                        .px(theme.spacing.sm)
                        .py(theme.spacing.xs)
                        .min_w_0()
                        .child(checkbox)
                        .children(option.detail.clone().map(|detail| {
                            // Under the label, past the box and its gap, so both read as one.
                            div().pl(px(16.) + theme.spacing.sm).child(
                                Label::new(detail)
                                    .size(TextSize::Small)
                                    .color(TextColor::Muted),
                            )
                        }))
                        .into_any_element()
                } else {
                    let mut row = ListItem::new(id, option.label.clone())
                        .icon(if chosen {
                            IconName::Check
                        } else {
                            IconName::ChevronRight
                        })
                        .selected(chosen)
                        .disabled(!active && !chosen);
                    if let Some(detail) = option.detail.clone() {
                        row = row.description(detail).description_lines(2);
                    }
                    if active && let Some(on_choose) = self.on_choose.clone() {
                        row =
                            row.on_click(move |_, window, cx| on_choose(&vec![index], window, cx));
                    }
                    row.into_any_element()
                }
            })
            .collect();
        let send = (self.multiple && active).then(|| {
            let on_choose = self.on_choose.clone();
            let picks = picked.clone();
            div().flex().justify_end().pt(theme.spacing.xs).child(
                Button::new(self.child("send"), strings.send.clone())
                    .icon(IconName::ArrowUp)
                    .size(ControlSize::Small)
                    .variant(ButtonVariant::Primary)
                    .disabled(picked.is_empty())
                    .on_click(move |_, window, cx| {
                        if let Some(on_choose) = &on_choose {
                            let mut picks = picks.clone();
                            picks.sort_unstable();
                            on_choose(&picks, window, cx);
                        }
                    }),
            )
        });
        crate::focus::selector(div().id(self.id.clone()), &self.id)
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .child(
                Card::new()
                    .p(theme.spacing.sm)
                    .gap(theme.spacing.xxs)
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(theme.spacing.xxs)
                            .px(theme.spacing.sm)
                            .py(theme.spacing.xs)
                            .child(
                                Label::new(self.question.clone())
                                    .weight(theme.typography.label_weight),
                            )
                            .when(self.multiple, |header| {
                                header.child(
                                    Label::new(strings.pick_any.clone())
                                        .size(TextSize::XSmall)
                                        .color(TextColor::Faint),
                                )
                            }),
                    )
                    .children(options)
                    .children(send),
            )
    }
}
