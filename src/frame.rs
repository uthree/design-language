//! The window frame: a title bar drawn by the application instead of the system.
//!
//! The title bar is the application's toolbar and the window's chrome at once: it carries the
//! document title and the app's controls, it is the drag handle, and on Windows and Linux it ends
//! in minimise, maximise and close. The platform still owns the behaviour — Windows hit-tests the
//! caption and the three buttons itself (so double-click, Aero Snap and the Windows 11 snap
//! layouts flyout keep working), and macOS keeps its own traffic lights.
//!
//! On Linux the window draws its own decorations: [`WindowFrame`] adds the resize edges, the
//! shadow and the rounded top corners the compositor would otherwise draw, and [`TitleBar`] the
//! window buttons. Where the system keeps its own frame — an X11 desktop without a compositor —
//! both step aside.
//!
//! ```ignore
//! let options = atelier::custom_frame(WindowOptions { ..Default::default() }, cx.theme());
//! cx.open_window(options, |_, cx| cx.new(|_| MyView));
//! // in MyView::render
//! WindowFrame::new()
//!     .flex_col()
//!     .child(TitleBar::new().title("Midnight Drive").child(play_button))
//!     .child(content)
//! ```

use gpui::{
    AnyElement, App, Bounds, BoxShadow, CursorStyle, Decorations, Div, HitboxBehavior,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
    ResizeEdge, SharedString, Size, StatefulInteractiveElement, StyleRefinement, Styled, Tiling,
    TitlebarOptions, Window, WindowControlArea, WindowDecorations, WindowOptions, canvas, div,
    point, prelude::FluentBuilder, px, svg,
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
/// Margin a client-decorated window keeps around its frame, for the shadow and the resize edges.
const FRAME_INSET: f32 = 10.;

/// Turn `options` into a window with a custom frame: the system title bar is hidden and, on
/// macOS, the traffic lights are centred on the theme's title bar.
///
/// Linux asks for client-side decorations, which [`WindowFrame`] and [`TitleBar`] draw. An X11
/// desktop without a compositor cannot show them and keeps its own frame; both then draw none.
pub fn custom_frame(options: WindowOptions, theme: &Theme) -> WindowOptions {
    let title = options
        .titlebar
        .as_ref()
        .and_then(|titlebar| titlebar.title.clone());
    let traffic_light_y = (theme.density.toolbar - px(TRAFFIC_LIGHT_HEIGHT)) / 2.;
    let decorations = if cfg!(any(target_os = "macos", target_os = "windows")) {
        WindowDecorations::Server
    } else {
        WindowDecorations::Client
    };
    WindowOptions {
        titlebar: Some(TitlebarOptions {
            title,
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), traffic_light_y)),
        }),
        window_decorations: Some(decorations),
        ..options
    }
}

/// Radius of a client-decorated window's top corners.
fn frame_radius(theme: &Theme) -> Pixels {
    theme.shape.radius_lg
}

/// The tiling of a window whose decorations are ours to draw, or `None` when the system draws
/// them.
fn client_tiling(window: &Window) -> Option<Tiling> {
    match window.window_decorations() {
        Decorations::Server => None,
        Decorations::Client { tiling } => Some(tiling),
    }
}

/// The root of a window opened with [`custom_frame`]: it paints the theme's window fill and, when
/// the window draws its own decorations, the frame around it.
///
/// Style it and add children as you would the root `div`. Under client-side decorations it keeps
/// a margin clear on every edge not tiled against the screen, drops a shadow into it, resizes the
/// window from it, and rounds the top corners; elsewhere it is a plain full-size `div`.
#[derive(IntoElement)]
pub struct WindowFrame {
    base: Div,
}

impl WindowFrame {
    /// An empty frame.
    pub fn new() -> Self {
        Self { base: div() }
    }
}

impl Default for WindowFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for WindowFrame {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for WindowFrame {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements)
    }
}

impl RenderOnce for WindowFrame {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let surface = self.base.size_full().bg(theme.window_fill());
        let Some(tiling) = client_tiling(window) else {
            return surface.into_any_element();
        };

        let inset = px(FRAME_INSET);
        window.set_client_inset(inset);
        let radius = frame_radius(theme);
        let rounded = !tiling.is_tiled();
        let shadow = BoxShadow {
            color: theme.colors.shadow.opacity(0.3),
            offset: point(px(0.), px(1.)),
            blur_radius: inset / 2.,
            spread_radius: px(0.),
        };

        div()
            .id("atelier-window-frame")
            .size_full()
            .when(!tiling.top, |frame| frame.pt(inset))
            .when(!tiling.bottom, |frame| frame.pb(inset))
            .when(!tiling.left, |frame| frame.pl(inset))
            .when(!tiling.right, |frame| frame.pr(inset))
            .child(
                canvas(
                    |_, window, _| {
                        let size = window.window_bounds().get_bounds().size;
                        window.insert_hitbox(
                            Bounds::new(point(px(0.), px(0.)), size),
                            HitboxBehavior::Normal,
                        )
                    },
                    move |_, hitbox, window, _| {
                        let size = window.window_bounds().get_bounds().size;
                        if let Some(edge) =
                            resize_edge(window.mouse_position(), inset, size, tiling)
                        {
                            window.set_cursor_style(resize_cursor(edge), &hitbox);
                        }
                    },
                )
                .size_full()
                .absolute(),
            )
            .on_mouse_move(|_, window, _| window.refresh())
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                let size = window.window_bounds().get_bounds().size;
                if let Some(edge) = resize_edge(event.position, inset, size, tiling) {
                    window.start_window_resize(edge);
                }
            })
            .child(
                surface
                    .cursor(CursorStyle::Arrow)
                    .border_color(theme.colors.border)
                    .when(!tiling.top, |surface| surface.border_t_1())
                    .when(!tiling.bottom, |surface| surface.border_b_1())
                    .when(!tiling.left, |surface| surface.border_l_1())
                    .when(!tiling.right, |surface| surface.border_r_1())
                    .when(rounded, |surface| {
                        surface
                            .rounded_tl(radius)
                            .rounded_tr(radius)
                            .shadow(vec![shadow])
                    })
                    // The margin's resize cursor ends at the frame.
                    .on_mouse_move(|_, _, cx| cx.stop_propagation()),
            )
            .into_any_element()
    }
}

/// The edge or corner of a window of `size` that `position` grabs, if it lies in the margin.
fn resize_edge(
    position: Point<Pixels>,
    inset: Pixels,
    size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    // A corner grabs a little further along each edge, so it is easier to hit.
    let corner = inset * 2.;
    let top = !tiling.top && position.y < inset;
    let bottom = !tiling.bottom && position.y > size.height - inset;
    let left = !tiling.left && position.x < inset;
    let right = !tiling.right && position.x > size.width - inset;
    let near_top = position.y < corner;
    let near_bottom = position.y > size.height - corner;
    let near_left = position.x < corner;
    let near_right = position.x > size.width - corner;
    let edge = if (top && near_left) || (left && near_top) {
        ResizeEdge::TopLeft
    } else if (top && near_right) || (right && near_top) {
        ResizeEdge::TopRight
    } else if (bottom && near_left) || (left && near_bottom) {
        ResizeEdge::BottomLeft
    } else if (bottom && near_right) || (right && near_bottom) {
        ResizeEdge::BottomRight
    } else if top {
        ResizeEdge::Top
    } else if bottom {
        ResizeEdge::Bottom
    } else if left {
        ResizeEdge::Left
    } else if right {
        ResizeEdge::Right
    } else {
        return None;
    };
    Some(edge)
}

fn resize_cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
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
        // Linux draws buttons only when the window draws its own decorations.
        let tiling = if linux { client_tiling(window) } else { None };
        let caption_buttons = windows || tiling.is_some();
        // Follow the frame's rounded top corners.
        let radius = tiling
            .filter(|tiling| !tiling.is_tiled())
            .map(|_| frame_radius(theme));
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
            .when_some(radius, |bar, radius| {
                bar.rounded_tl(radius).rounded_tr(radius)
            })
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
                .on_mouse_down(MouseButton::Right, |event, window, _| {
                    window.show_window_menu(event.position);
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
            .when(caption_buttons, |bar| bar.child(CaptionButtons { radius }))
    }
}

/// Minimise, maximise/restore and close, in the Windows 11 arrangement.
#[derive(IntoElement)]
struct CaptionButtons {
    /// The window's top-corner radius, which the close button's hover follows.
    radius: Option<Pixels>,
}

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
                .when_some(self.radius.filter(|_| close), |button, radius| {
                    button.rounded_tr(radius)
                })
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
