//! A swatch opens a picker whose sliders, code and presets make the colour, and the hue it was
//! given survives a trip through grey.

use std::cell::RefCell;
use std::rc::Rc;

use atelier::{ColorPicker, ColorSwatch, Hsv, Theme, WindowFrame, hex};
use gpui::{
    Context, Hsla, IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext,
    Window, div, px, rgb,
};

struct Panel {
    color: Rc<RefCell<Hsla>>,
    open: bool,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = *self.color.borrow();
        let store = self.color.clone();
        WindowFrame::new().child(
            div()
                .w(px(360.))
                .flex()
                .flex_col()
                .child(
                    ColorSwatch::new("swatch", color)
                        .selected(self.open)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open = !this.open;
                            cx.notify();
                        })),
                )
                .children(self.open.then(|| {
                    ColorPicker::new("picker", color)
                        .presets([rgb(0xffffff).into(), rgb(0x101010).into()])
                        .on_change(cx.listener(move |_, new: &Hsla, _, cx| {
                            *store.borrow_mut() = *new;
                            cx.notify();
                        }))
                })),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Rc<RefCell<Hsla>>, &mut VisualTestContext) {
    cx.update(|cx| {
        atelier::init(cx, Theme::default());
        atelier::motion::reduce_motion(cx);
    });
    let color = Rc::new(RefCell::new(rgb(0x3d7bff).into()));
    let (_, window) = cx.add_window_view({
        let color = color.clone();
        |_, _| Panel { color, open: false }
    });
    window.run_until_parked();
    (color, window)
}

#[gpui::test]
fn a_swatch_opens_a_picker_whose_parts_make_the_colour(cx: &mut TestAppContext) {
    let (color, window) = open(cx);
    assert!(window.debug_bounds("picker").is_none());
    let swatch = window.debug_bounds("swatch").unwrap();
    window.simulate_click(swatch.center(), Default::default());
    window.run_until_parked();
    assert!(
        window.debug_bounds("picker").is_some(),
        "the swatch opens it"
    );

    // A preset is pressed.
    let preset = window.debug_bounds("picker-preset-0").unwrap();
    window.simulate_click(preset.center(), Default::default());
    window.run_until_parked();
    assert_eq!(hex(*color.borrow()), "#FFFFFF");

    // The code is typed.
    let code = window.debug_bounds("picker-code").unwrap();
    window.simulate_click(code.center(), Default::default());
    window.simulate_keystrokes("cmd-a backspace");
    window.simulate_keystrokes("ctrl-a backspace");
    window.simulate_input("#38f");
    window.simulate_keystrokes("enter");
    window.run_until_parked();
    assert_eq!(hex(*color.borrow()), "#3388FF");

    // Saturation to nothing is grey; back up again, the hue is the one it had.
    let saturation = window.debug_bounds("picker-saturation").unwrap();
    window.simulate_click(
        saturation.origin + gpui::point(px(1.), px(4.)),
        Default::default(),
    );
    window.run_until_parked();
    let grey = hex(*color.borrow());
    assert_eq!(&grey[1..3], &grey[3..5], "grey: {grey}");
    assert_eq!(&grey[3..5], &grey[5..7], "grey: {grey}");
    let right = saturation.origin + gpui::point(saturation.size.width - px(1.), px(4.));
    window.simulate_click(right, Default::default());
    window.run_until_parked();
    // Full saturation at the far end, at the hue it had before it went grey.
    let back = Hsv::from_color(*color.borrow());
    let before = Hsv::from_color(rgb(0x3388ff).into());
    assert!((back.hue - before.hue).abs() < 1., "{back:?} vs {before:?}");
    assert!(back.saturation > 0.99, "{back:?}");
}
