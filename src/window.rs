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
//! On Windows, gpui blurs through the accent policy DWM applies to the window, and DWM drops that
//! accent while it plays the maximize, restore and snap transitions: for the length of the
//! animation the window is plainly transparent. [`set_window_background`] turns those
//! transitions off for a blurred window, so it changes size at once and stays frosted.
//!
//! Call it again whenever a different theme is installed: leaving a translucent material and
//! coming back to it makes gpui build a fresh blur view.
//!
//! On Linux, gpui can blur only through KWin's blur protocol on Wayland; on X11 and on every other
//! Wayland compositor a blurred window comes out plainly transparent, the desktop sharp behind
//! the chrome. There [`Theme::window_background`] asks for an opaque window and
//! [`Theme::window_fill`] paints the theme's background under the chrome instead. A window with
//! client-side decorations is asked to be transparent either way, so the margin around the frame
//! where [`crate::WindowFrame`] draws its shadow stays clear.

use gpui::{Window, WindowBackgroundAppearance};

use crate::theme::Theme;

/// Give `window` the background `theme` asks for — opaque, or translucent over a blurred desktop.
///
/// Use it in place of `Window::set_background_appearance`, both when the window opens and after
/// every theme change.
pub fn set_window_background(window: &mut Window, theme: &Theme) {
    let mut background = theme.window_background();
    if background == WindowBackgroundAppearance::Opaque
        && matches!(
            window.window_decorations(),
            gpui::Decorations::Client { .. }
        )
    {
        // The frame paints the opaque fill itself, inside the shadow margin.
        background = WindowBackgroundAppearance::Transparent;
    }
    window.set_background_appearance(background);
    #[cfg(target_os = "macos")]
    macos::restore_blur_material(window);
    #[cfg(target_os = "windows")]
    windows::keep_blur_through_transitions(window, background);
}

/// Whether a window asking for `WindowBackgroundAppearance::Blurred` actually gets a blurred
/// backdrop on this system.
pub(crate) fn blur_supported() -> bool {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            // gpui blurs through `org_kde_kwin_blur`, which only KWin offers.
            gpui::guess_compositor() == "Wayland"
                && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktops| {
                    desktops
                        .split(':')
                        .any(|desktop| desktop.eq_ignore_ascii_case("KDE"))
                })
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    {
        true
    }
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

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;

    use gpui::{Window, WindowBackgroundAppearance};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// `DWMWA_TRANSITIONS_FORCEDISABLED`: turns off the window's open, close, minimize, maximize
    /// and restore animations.
    const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }

    /// Turn DWM's window transitions off while the window is blurred, and back on otherwise.
    ///
    /// DWM draws a transition from a snapshot of the window without its acrylic accent, so a
    /// double-click on the title bar would show the desktop sharp behind the chrome until the
    /// animation ends.
    pub(super) fn keep_blur_through_transitions(
        window: &Window,
        background: WindowBackgroundAppearance,
    ) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        // A Win32 `BOOL`.
        let disabled = i32::from(background == WindowBackgroundAppearance::Blurred);
        unsafe {
            DwmSetWindowAttribute(
                handle.hwnd.get(),
                DWMWA_TRANSITIONS_FORCEDISABLED,
                (&raw const disabled).cast(),
                size_of::<i32>() as u32,
            );
        }
    }
}
