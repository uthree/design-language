//! The window background: the material the system paints behind translucent chrome.
//!
//! Glass asks the window for a blurred background, so the chrome's tint sits over a frosted
//! backdrop instead of the desktop. gpui arranges that per platform, but on macOS 26 and later
//! the material it picks — `NSVisualEffectMaterial.selection` — no longer draws a backdrop, and
//! the window comes out clear. [`set_window_background`] asks for the theme's background and
//! then puts the blur back on a material that still blurs.
//!
//! ```ignore
//! cx.open_window(options, |window, cx| {
//!     atelier::set_window_background(window, cx.theme());
//!     cx.new(|_| MyView)
//! });
//! ```
//!
//! Call it again whenever a different theme is installed: leaving a translucent material and
//! coming back to it makes gpui build a fresh blur view.

use gpui::Window;

use crate::theme::Theme;

/// Give `window` the background `theme` asks for — opaque, or translucent over a blurred desktop.
///
/// Use it in place of `Window::set_background_appearance`, both when the window opens and after
/// every theme change.
pub fn set_window_background(window: &mut Window, theme: &Theme) {
    window.set_background_appearance(theme.window_background());
    #[cfg(target_os = "macos")]
    macos::restore_blur_material(window);
}

#[cfg(target_os = "macos")]
mod macos {
    use gpui::Window;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// `NSVisualEffectMaterialUnderWindowBackground`: what AppKit itself puts behind a window's
    /// background, and one of the materials that still blurs under Liquid Glass.
    const UNDER_WINDOW_BACKGROUND: isize = 21;

    /// Move the blur view gpui installed onto a material that draws a backdrop.
    ///
    /// Does nothing when the window has no blur view: an opaque theme, or a macOS old enough that
    /// gpui blurs through the window server instead.
    pub(super) fn restore_blur_material(window: &Window) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        let Some(effect_view_class) = AnyClass::get(c"NSVisualEffectView") else {
            return;
        };
        let view = handle.ns_view.as_ptr().cast::<AnyObject>();

        unsafe {
            let native_window: *mut AnyObject = msg_send![view, window];
            if native_window.is_null() {
                return;
            }
            let content_view: *mut AnyObject = msg_send![native_window, contentView];
            if content_view.is_null() {
                return;
            }
            let subviews: *mut AnyObject = msg_send![content_view, subviews];
            if subviews.is_null() {
                return;
            }

            let count: usize = msg_send![subviews, count];
            for index in 0..count {
                let subview: *mut AnyObject = msg_send![subviews, objectAtIndex: index];
                let is_effect_view: bool = msg_send![subview, isKindOfClass: effect_view_class];
                if is_effect_view {
                    let _: () = msg_send![subview, setMaterial: UNDER_WINDOW_BACKGROUND];
                }
            }
        }
    }
}
