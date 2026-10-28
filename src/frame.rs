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

use crate::focus::{set_focus_visible, tab_navigation};
use crate::icon::IconName;
use crate::text::Label;
use crate::theme::{ActiveTheme, Theme};

/// Width of the space macOS's traffic lights occupy at the leading edge.
const TRAFFIC_LIGHTS_WIDTH: f32 = 80.;
/// Height of a traffic light, for centring them in the title bar.
const TRAFFIC_LIGHT_HEIGHT: f32 = 14.;
/// Width of one Windows-style caption button.
const CAPTION_BUTTON_WIDTH: f32 = 46.;
/// Narrowest the title shrinks to when the title bar runs out of room.
const MIN_TITLE_WIDTH: f32 = 72.;
/// Shrink factor of what gives way before the title truncates: the room the title bar's sides
/// keep to centre the title.
const YIELD_SHRINK: f32 = 1000.;
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
    let traffic_light_y = (theme.density.title_bar - px(TRAFFIC_LIGHT_HEIGHT)) / 2.;
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
///
/// It is also where keyboard navigation starts: it holds the window's root focus, so Tab and
/// Shift-Tab ([`crate::FocusNext`], [`crate::FocusPrevious`]) reach every control from the
/// moment the window opens, and a press anywhere hides the focus ring until the keyboard moves
/// focus again.
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
        // The window's root focus: not a tab stop itself, but where Tab starts from, and where
        // focus returns when the person clicks somewhere that takes no focus of its own.
        let root = window
            .use_keyed_state("atelier-window-root", cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        if window.focused(cx).is_none() {
            let root = root.clone();
            window.defer(cx, move |window, _| window.focus(&root));
        }
        let theme = cx.theme();
        // The root focus sits on an empty child rather than on the surface itself: an element
        // that tracks focus takes focus on every press inside it and marks the press handled,
        // and a handled press on the title bar is one the platform no longer starts a window
        // move from. Actions still bubble from the child to the surface, and the surface hands
        // focus back to it from a press nothing else claimed, without marking that press.
        let anchor = div().absolute().size_0().track_focus(&root);
        let surface = tab_navigation(self.base)
            .capture_any_mouse_down(|_, _, cx| set_focus_visible(false, cx))
            .on_any_mouse_down(move |_, window, _| {
                if !window.default_prevented() {
                    window.focus(&root);
                }
            })
            .child(anchor)
            .size_full()
            .bg(theme.window_fill());
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
/// One slim row: [`TitleBar::leading`] controls and the children (`.child`) at the leading edge,
/// the title centred on the window, and [`TitleBar::trailing`] controls at the far end. Its own
/// surface drags the window, and each control is shielded from the drag, so buttons and
/// segmented controls stay clickable. A [`TitleBar::title`] is not shielded: grabbing the title
/// moves the window, as people expect.
///
/// The title stays on the window's centre line while the controls on both sides leave room for
/// it; when one side is wider it slides away from that side instead of covering it, and then
/// truncates.
///
/// The title is all the bar says: there is no subtitle. A save state, a path or a count belongs
/// in the window's content or its status bar, not beside the title.
#[derive(IntoElement)]
pub struct TitleBar {
    base: Div,
    title: Option<SharedString>,
    leading: Vec<AnyElement>,
    children: Vec<AnyElement>,
    trailing: Vec<AnyElement>,
    window_controls: Option<(Pixels, AnyElement)>,
    window_drag: bool,
}

impl TitleBar {
    /// An empty title bar.
    pub fn new() -> Self {
        Self {
            base: div(),
            title: None,
            leading: Vec::new(),
            children: Vec::new(),
            trailing: Vec::new(),
            window_controls: None,
            window_drag: true,
        }
    }

    /// The document or window title, centred on the window.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
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

    /// The application's own minimise, maximise and close, `width` wide, in place of the ones
    /// the title bar draws on Windows and Linux. For an application that hit-tests its caption
    /// buttons natively. The title stays centred, measured against `width`. Ignored on macOS,
    /// where the traffic lights are the system's.
    pub fn window_controls(mut self, width: Pixels, controls: impl IntoElement) -> Self {
        self.window_controls = Some((width, controls.into_any_element()));
        self
    }

    /// Whether the title bar makes its free space the window's drag area (the default). Turn
    /// it off when the application hit-tests the drag region itself — for Windows snapping and
    /// double-click maximise through its own `WM_NCHITTEST` — and the title bar only lays out.
    pub fn window_drag(mut self, drag: bool) -> Self {
        self.window_drag = drag;
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
        let window_controls = self.window_controls.filter(|_| caption_buttons);
        let drag = self.window_drag;
        // Follow the frame's rounded top corners.
        let radius = tiling
            .filter(|tiling| !tiling.is_tiled())
            .map(|_| frame_radius(theme));
        let height = theme.density.title_bar;

        let heading = self.title.map(|title| {
            // One line, truncating down to a few characters when the bar runs out of room.
            div()
                .flex()
                .flex_shrink()
                .min_w(px(MIN_TITLE_WIDTH))
                .overflow_hidden()
                .child(
                    Label::new(title)
                        .weight(theme.typography.label_weight)
                        .truncate(),
                )
        });

        // The edges the system's buttons take: the traffic lights at the leading end, the caption
        // buttons at the trailing one, or a margin.
        let leading_edge = if traffic_lights {
            px(TRAFFIC_LIGHTS_WIDTH)
        } else {
            theme.spacing.md
        };
        let trailing_edge = if let Some((width, _)) = &window_controls {
            *width + theme.spacing.sm
        } else if caption_buttons {
            px(CAPTION_BUTTON_WIDTH * 3.) + theme.spacing.sm
        } else {
            theme.spacing.md
        };
        // The two sides grow by the same amount from these bases, which make up for the uneven
        // edges, so the title sits on the window's centre line. A side whose controls need more
        // keeps them and pushes the title over, which then truncates. The sides carry no padding
        // of their own: a basis smaller than a side's padding is raised to it, and the title
        // would drift off the centre line by the difference.
        let mut leading_side = div();
        let mut trailing_side = div();
        for side in [&mut leading_side, &mut trailing_side] {
            side.style().flex_shrink = Some(YIELD_SHRINK);
        }
        let leading_side = leading_side
            .flex()
            .flex_grow()
            .flex_basis((trailing_edge - leading_edge).max(px(0.)))
            .h_full()
            .items_center()
            .gap(theme.spacing.sm)
            .children(
                self.leading
                    .into_iter()
                    .map(|child| div().flex().flex_none().occlude().child(child)),
            )
            // Each child blocks the drag area under it, so its clicks reach it.
            .children(
                self.children
                    .into_iter()
                    .map(|child| div().flex().flex_none().occlude().child(child)),
            );
        let trailing_side = trailing_side
            .flex()
            .flex_grow()
            .flex_basis((leading_edge - trailing_edge).max(px(0.)))
            .h_full()
            .items_center()
            .justify_end()
            .gap(theme.spacing.sm)
            .children(
                self.trailing
                    .into_iter()
                    .map(|child| div().flex().flex_none().occlude().child(child)),
            );
        self.base
            .id("atelier-title-bar")
            .flex()
            .flex_none()
            .items_center()
            .w_full()
            .h(height)
            .bg(title_bar_fill(theme))
            .when_some(radius, |bar, radius| {
                bar.rounded_tl(radius).rounded_tr(radius)
            })
            .border_b_1()
            .border_color(theme.colors.border)
            .when(drag, |bar| bar.window_control_area(WindowControlArea::Drag))
            .when(mac && drag, |bar| {
                bar.on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
            })
            .when(linux && caption_buttons && drag, |bar| {
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
            .child(div().flex_none().w(leading_edge))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap(theme.spacing.md)
                    // When even the shortest title leaves too little room, the row overflows at
                    // its leading end: the leading children are cut off before anything reaches
                    // the trailing controls or the traffic lights.
                    .justify_end()
                    .overflow_hidden()
                    .child(leading_side)
                    .children(heading)
                    .child(trailing_side),
            )
            .map(|bar| match window_controls {
                Some((width, controls)) => bar
                    .child(div().flex_none().w(theme.spacing.sm))
                    .child(div().flex().flex_none().w(width).h_full().child(controls)),
                None if caption_buttons => bar
                    .child(div().flex_none().w(theme.spacing.sm))
                    .child(CaptionButtons { radius }),
                None => bar.child(div().flex_none().w(trailing_edge)),
            })
    }
}

/// The title bar's fill: the chrome tint, or on Windows under the acrylic system backdrop the
/// same tint composited over the background and made opaque, so the caption buttons DWM draws
/// in the extended frame do not show through (see `native_caption_shows_through`).
fn title_bar_fill(theme: &Theme) -> gpui::Hsla {
    let chrome = theme.colors.chrome;
    if crate::window::native_caption_shows_through(theme) {
        let opaque = |color: gpui::Hsla| gpui::Hsla { a: 1., ..color };
        opaque(crate::theme::mix(
            opaque(theme.colors.background),
            opaque(chrome),
            chrome.a,
        ))
    } else {
        chrome
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
