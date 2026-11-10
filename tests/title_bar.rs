//! The title bar's app icon: first at the leading edge on Windows and Linux, absent on macOS,
//! at the bar's icon size, and the title stays on the window's centre line.

use atelier::{Button, IconName, IconSource, Theme, TitleBar, WindowFrame, title_bar_icon_size};
use gpui::{
    Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window, div, px, size,
};

struct Bar;

impl Render for Bar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new().size_full().child(
            TitleBar::new()
                .app_icon(IconSource::image("app/title-bar.png"))
                .leading(Button::icon_only("sidebar", IconName::SidebarLeft))
                .title("Document")
                .child(div().w(px(120.))),
        )
    }
}

#[gpui::test]
fn the_app_icon_comes_first_and_the_title_stays_centred(cx: &mut TestAppContext) {
    let theme = Theme::default();
    let side = title_bar_icon_size(&theme);
    cx.update(|cx| atelier::init(cx, theme));
    let (_, window) = cx.add_window_view(|_, _| Bar);
    window.simulate_resize(size(px(1000.), px(600.)));
    window.run_until_parked();

    let button = window.debug_bounds("sidebar").expect("the sidebar toggle");
    let title = window.debug_bounds("atelier-title").expect("the title");
    let centre = title.left() + title.size.width / 2.;
    assert!(
        (centre - px(500.)).abs() < px(1.),
        "title centred: {title:?}"
    );
    if cfg!(target_os = "macos") {
        // The traffic lights hold the corner; Mac windows carry no app icon.
        assert!(window.debug_bounds("atelier-app-icon").is_none());
        return;
    }
    let icon = window
        .debug_bounds("atelier-app-icon")
        .expect("the app icon");
    assert_eq!(icon.size.width, side);
    assert_eq!(icon.size.height, side);
    assert!(icon.right() <= button.left(), "{icon:?} before {button:?}");
    assert!(icon.left() < px(100.), "at the top left: {icon:?}");
}
