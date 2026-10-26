//! The window frame: a title bar drawn by the application instead of the system.
//!
//! The title bar is the application's toolbar and the window's chrome at once: it carries the
//! document title and the app's controls, it is the drag handle, and on Windows and Linux it ends
//! in minimise, maximise and close. The platform still owns the behaviour — Windows hit-tests the
//! caption and the three buttons itself (so double-click, Aero Snap and the Windows 11 snap
//! layouts flyout keep working), and macOS keeps its own traffic lights.
//!
//! ```ignore
//! let options = atelier::custom_frame(WindowOptions { ..Default::default() }, cx.theme());
//! cx.open_window(options, |_, cx| cx.new(|_| MyView));
//! // in MyView::render
//! TitleBar::new().title("Midnight Drive").child(play_button)
//! ```

use gpui::{
    AnyElement, App, Decorations, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled, TitlebarOptions,
    Window, WindowControlArea, WindowDecorations, WindowOptions, div, point,
    prelude::FluentBuilder, px, svg,
};

use crate::icon::IconName;
use crate::text::Label;
use crate::theme::{ActiveTheme, Theme};

/// Width of the space macOS's traffic lights occupy at the leading edge.
const TRAFFIC_LIGHTS_WIDTH: f32 = 80.;
/// Height of a traffic light, for centring them in the title bar.
const TRAFFIC_LIGHT_HEIGHT: f32 = 14.;
/// Width of one Windows-style caption button.
const CAPTION_BUTTON_WIDTH: f32 = 46.;

/// Turn `options` into a window with a custom frame: the system title bar is hidden and, on
/// macOS, the traffic lights are centred on the theme's title bar.
///
/// Linux keeps server-side decorations: gpui 0.2.2 leaves resize edges and shadows of a
/// client-decorated window to the application, which Atelier does not draw yet. [`TitleBar`]
/// then shows no window buttons, because the system frame already has them.
pub fn custom_frame(options: WindowOptions, theme: &Theme) -> WindowOptions {
    let title = options
        .titlebar
        .as_ref()
        .and_then(|titlebar| titlebar.title.clone());
    let traffic_light_y = (theme.density.toolbar - px(TRAFFIC_LIGHT_HEIGHT)) / 2.;
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title,
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), traffic_light_y)),
        }),
        window_decorations: Some(WindowDecorations::Server),
        ..options
    }
}

/// The title bar of a window opened with [`custom_frame`].
///
/// Its own surface drags the window; children (`.child`) sit after the title, [`TitleBar::trailing`]
/// elements at the far end, and each one is shielded from the drag, so buttons and segmented controls stay clickable. A [`TitleBar::title`] is not
/// shielded: grabbing the title moves the window, as people expect.
#[derive(IntoElement)]
pub struct TitleBar {
    base: Div,
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    leading: Vec<AnyElement>,
    children: Vec<AnyElement>,
    trailing: Vec<AnyElement>,
}

impl TitleBar {
    /// An empty title bar.
    pub fn new() -> Self {
        Self {
            base: div(),
            title: None,
            subtitle: None,
            leading: Vec::new(),
            children: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// The document or window title at the leading edge.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A muted line under the title, such as the save state.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// A control before the title, such as the sidebar toggle.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading.push(element.into_any_element());
        self
    }

    /// A control at the trailing end, before the window buttons. The space between the leading
    /// children and the trailing ones drags the window.
    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
        self
    }
}

impl Default for TitleBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for TitleBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements)
    }
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mac = cfg!(target_os = "macos");
        let windows = cfg!(target_os = "windows");
        let linux = !mac && !windows;
        let traffic_lights = mac && !window.is_fullscreen();
        // Linux draws buttons only when the compositor left decorations to the client.
        let caption_buttons =
            windows || (linux && matches!(window.window_decorations(), Decorations::Client { .. }));
        let height = theme.density.toolbar;

        let heading = self.title.map(|title| {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .child(Label::new(title).weight(theme.typography.label_weight))
                .children(self.subtitle.map(|subtitle| {
                    Label::new(subtitle)
                        .size(crate::TextSize::XSmall)
                        .color(crate::TextColor::Faint)
                }))
        });

        self.base
            .id("atelier-title-bar")
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(height)
            .bg(theme.colors.chrome)
            .border_b_1()
            .border_color(theme.colors.border)
            .window_control_area(WindowControlArea::Drag)
            .pl(if traffic_lights {
                px(TRAFFIC_LIGHTS_WIDTH)
            } else {
                theme.spacing.md
            })
            .when(!caption_buttons, |bar| bar.pr(theme.spacing.md))
            .when(mac, |bar| {
                bar.on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
            })
            .when(linux && caption_buttons, |bar| {
                bar.on_mouse_down(MouseButton::Left, |event, window, _| {
                    if event.click_count == 2 {
                        window.zoom_window();
                    } else {
                        window.start_window_move();
                    }
                })
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .children(
                        self.leading
                            .into_iter()
                            .map(|child| div().flex().flex_none().occlude().child(child)),
                    )
                    .children(heading)
                    // Each child blocks the drag area under it, so its clicks reach it.
                    .children(
                        self.children
                            .into_iter()
                            .map(|child| div().flex().flex_none().occlude().child(child)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .h_full()
                    .items_center()
                    .gap(theme.spacing.sm)
                    .children(
                        self.trailing
                            .into_iter()
                            .map(|child| div().flex().flex_none().occlude().child(child)),
                    ),
            )
            .when(caption_buttons, |bar| bar.child(CaptionButtons))
    }
}

/// Minimise, maximise/restore and close, in the Windows 11 arrangement.
#[derive(IntoElement)]
struct CaptionButtons;

impl RenderOnce for CaptionButtons {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let windows = cfg!(target_os = "windows");
        let glyph = if window.is_window_active() {
            theme.colors.text
        } else {
            theme.colors.text_faint
        };
        let maximise = if window.is_maximized() {
            IconName::WindowRestore
        } else {
            IconName::WindowMaximize
        };
        let button = |id: &'static str, icon: IconName, area: WindowControlArea| {
            let close = area == WindowControlArea::Close;
            let hover = if close {
                theme.colors.danger
            } else {
                theme.colors.hover
            };
            let hover_glyph = if close { theme.text_on(hover) } else { glyph };
            div()
                .id(id)
                .group(id)
                .flex()
                .items_center()
                .justify_center()
                .w(px(CAPTION_BUTTON_WIDTH))
                .h_full()
                .occlude()
                .hover(move |style| style.bg(hover))
                .child(
                    svg()
                        .path(icon.path())
                        .size(px(10.))
                        .text_color(glyph)
                        .group_hover(id, move |style| style.text_color(hover_glyph)),
                )
                // Windows handles the click from its own hit test; elsewhere the click is ours.
                .when(windows, |button| button.window_control_area(area))
                .when(!windows, |button| {
                    button.on_click(move |_, window, _| match area {
                        WindowControlArea::Min => window.minimize_window(),
                        WindowControlArea::Max => window.zoom_window(),
                        WindowControlArea::Close => window.remove_window(),
                        WindowControlArea::Drag => {}
                    })
                })
        };
        div()
            .flex()
            .flex_none()
            .h_full()
            .child(button(
                "atelier-minimize",
                IconName::WindowMinimize,
                WindowControlArea::Min,
            ))
            .child(button("atelier-maximize", maximise, WindowControlArea::Max))
            .child(button(
                "atelier-close",
                IconName::WindowClose,
                WindowControlArea::Close,
            ))
    }
}
