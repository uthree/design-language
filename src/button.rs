//! Buttons: the one primary action, everything else, and icon-only tools.

use gpui::{
    AnyView, App, ClickEvent, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, linear_color_stop,
    linear_gradient, px,
};

use crate::icon::{Icon, IconName};
use crate::overlay::Tooltip;
use crate::theme::{ActiveTheme, PrimaryStyle, Theme, mix};

/// What a button means, which decides how loudly it is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// The one action a group exists for. Accent- or solid-filled, per [`PrimaryStyle`].
    Primary,
    /// An ordinary action: a raised, bordered control.
    #[default]
    Secondary,
    /// A quiet action that only shows its fill under the pointer. Toolbars use these.
    Ghost,
    /// An action that destroys work.
    Danger,
    /// An action that hands work to the AI. Carries the agent colour.
    Agent,
}

/// Control height, from [`crate::Density`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    /// Inline toolbars and table cells.
    Small,
    /// The default.
    #[default]
    Medium,
    /// Calls to action.
    Large,
}

impl ControlSize {
    /// Height under `theme`.
    pub fn height(self, theme: &Theme) -> gpui::Pixels {
        match self {
            Self::Small => theme.density.control_sm,
            Self::Medium => theme.density.control_md,
            Self::Large => theme.density.control_lg,
        }
    }

    fn icon(self) -> gpui::Pixels {
        match self {
            Self::Small => px(13.),
            Self::Medium => px(15.),
            Self::Large => px(17.),
        }
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A push button, optionally latching (`selected`) like a mute or solo switch.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    icon_after: Option<IconName>,
    variant: ButtonVariant,
    size: ControlSize,
    selected: bool,
    tint: Option<Hsla>,
    disabled: bool,
    full_width: bool,
    square: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl Button {
    /// A labelled button.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            label: Some(label.into()),
            ..Self::bare(id)
        }
    }

    /// An icon-only button. Give it a [`Button::tooltip`]: an icon alone does not say what it does.
    pub fn icon_only(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            icon: Some(icon),
            square: true,
            variant: ButtonVariant::Ghost,
            ..Self::bare(id)
        }
    }

    fn bare(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            icon_after: None,
            variant: ButtonVariant::default(),
            size: ControlSize::default(),
            selected: false,
            tint: None,
            disabled: false,
            full_width: false,
            square: false,
            tooltip: None,
            on_click: None,
        }
    }

    /// What the button means.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Shorthand for [`ButtonVariant::Primary`].
    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    /// Shorthand for [`ButtonVariant::Ghost`].
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// Control height.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Leading icon.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Trailing icon, usually a chevron on a menu button.
    pub fn icon_after(mut self, icon: IconName) -> Self {
        self.icon_after = Some(icon);
        self
    }

    /// Latched on. Draws the selected fill, or [`Button::tint`] if one is given.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The colour a latched button fills with: yellow for solo, blue for mute, red for arm.
    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    /// Greyed out and unclickable.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Stretch to the container's width.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Hover text.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Called when the button is clicked.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

/// Fill, hover fill, text colour and border of one button state.
struct Paint {
    fill: Hsla,
    hover: Hsla,
    text: Hsla,
    border: Option<Hsla>,
}

fn paint(theme: &Theme, variant: ButtonVariant, selected: bool, tint: Option<Hsla>) -> Paint {
    let colors = &theme.colors;
    let tone = if theme.appearance.is_dark() {
        gpui::white()
    } else {
        gpui::black()
    };
    let filled = |fill: Hsla| Paint {
        fill,
        hover: mix(fill, tone, 0.1),
        text: theme.text_on(fill),
        border: None,
    };
    if selected {
        return match tint {
            Some(tint) => filled(tint),
            None => Paint {
                fill: colors.selected,
                hover: mix(colors.selected, tone, 0.06),
                text: colors.accent_text,
                border: Some(colors.border),
            },
        };
    }
    match variant {
        ButtonVariant::Primary => match theme.primary_style {
            PrimaryStyle::Accent => Paint {
                text: colors.on_accent,
                hover: colors.accent_hover,
                ..filled(colors.accent)
            },
            PrimaryStyle::Solid => Paint {
                text: colors.on_solid,
                ..filled(colors.solid)
            },
        },
        ButtonVariant::Danger => filled(colors.danger),
        ButtonVariant::Agent => Paint {
            fill: colors.agent_wash,
            hover: colors.agent.opacity(0.24),
            text: colors.agent,
            border: Some(colors.agent.opacity(0.35)),
        },
        ButtonVariant::Secondary => Paint {
            fill: colors.elevated,
            hover: mix(colors.elevated, tone, 0.06),
            text: colors.text,
            border: Some(colors.border_strong),
        },
        ButtonVariant::Ghost => Paint {
            fill: gpui::transparent_black(),
            hover: colors.hover,
            text: colors.text_muted,
            border: None,
        },
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let height = self.size.height(theme);
        let paint = paint(theme, self.variant, self.selected, self.tint);
        let raised = matches!(self.variant, ButtonVariant::Secondary) && !self.selected;

        let mut button = div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(match self.size {
                ControlSize::Small => theme.spacing.xs,
                _ => theme.spacing.sm - theme.spacing.xxs,
            })
            .h(height)
            .rounded(theme.control_radius(height))
            .text_size(match self.size {
                ControlSize::Small => theme.typography.size_sm,
                _ => theme.typography.size_md,
            })
            .font_weight(theme.typography.label_weight)
            .text_color(paint.text)
            .whitespace_nowrap();

        button = if self.square {
            button.w(height)
        } else {
            button.px(match self.size {
                ControlSize::Small => theme.density.control_padding * 0.75,
                ControlSize::Medium => theme.density.control_padding,
                ControlSize::Large => theme.density.control_padding * 1.4,
            })
        };
        if self.full_width {
            button = button.w_full();
        }

        // Studio draws raised controls with a top-lit gradient, the way Logic's buttons catch
        // the light; the flatter directions use the plain fill.
        button = if raised && theme.colors.highlight.a > 0. && !self.disabled {
            let top = mix(
                paint.fill,
                gpui::white(),
                if theme.appearance.is_dark() {
                    0.05
                } else {
                    0.5
                },
            );
            button.bg(linear_gradient(
                180.,
                linear_color_stop(top, 0.),
                linear_color_stop(paint.fill, 1.),
            ))
        } else {
            button.bg(paint.fill)
        };
        if let Some(border) = paint.border {
            button = button.border_1().border_color(border);
        }
        // Only the bezelled direction lifts its controls; elsewhere a hairline is enough.
        if raised && theme.colors.highlight.a > 0. {
            button = button.shadow(theme.elevation.low.clone());
        }

        if let Some(icon) = self.icon {
            button = button.child(Icon::new(icon).size(self.size.icon()).color(paint.text));
        }
        if let Some(label) = self.label {
            button = button.child(label);
        }
        if let Some(icon) = self.icon_after {
            button = button.child(
                Icon::new(icon)
                    .size(self.size.icon() - px(2.))
                    .color(paint.text.opacity(0.7)),
            );
        }

        if self.disabled {
            return button.opacity(0.45);
        }
        let hover = paint.hover;
        let pressed = mix(hover, paint.text, 0.08);
        button = button
            .hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed));
        if let Some(tooltip) = self.tooltip {
            button = button.tooltip(move |window, cx| -> AnyView {
                Tooltip::new(tooltip.clone()).build(window, cx)
            });
        }
        if let Some(handler) = self.on_click {
            button = button.on_click(handler);
        }
        button
    }
}
