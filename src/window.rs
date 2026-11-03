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
//! On Windows, gpui blurs through the acrylic accent policy, which DWM drops while it plays the
//! maximize, restore and snap animations: for the length of the animation the window is plainly
//! transparent. On Windows 11 22H2 and later [`set_window_background`] asks DWM for its own
//! acrylic system backdrop instead, which stays through those animations. Like every system
//! backdrop it gives way to a plain fill while the window is inactive. Older Windows keeps the
//! accent.
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
    #[cfg(target_os = "windows")]
    let (background, system_backdrop) =
        windows::apply_system_backdrop(window, background, theme.appearance.is_dark());
    window.set_background_appearance(background);
    #[cfg(target_os = "windows")]
    if system_backdrop {
        windows::disable_accent(window);
    }
    #[cfg(target_os = "macos")]
    macos::restore_blur_material(window);
}

/// Whether DWM draws its own caption buttons inside the window under `theme`, where they show
/// through translucent chrome.
///
/// The acrylic system backdrop needs the frame extended over the whole client area, and DWM
/// then draws the native minimise, maximise and close in the extended frame, under gpui's
/// rendering. Through Glass's translucent title bar they show faintly behind the title bar's
/// own buttons, and in a maximised window, where the two sets sit a few pixels apart, the glyphs
/// look doubled. [`crate::TitleBar`] paints itself opaque when this is true.
pub(crate) fn native_caption_shows_through(theme: &Theme) -> bool {
    #[cfg(target_os = "windows")]
    {
        theme.blurs_window() && windows::has_system_backdrop()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = theme;
        false
    }
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
    use std::sync::OnceLock;

    use gpui::{Window, WindowBackgroundAppearance};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    /// The first build with `DWMWA_SYSTEMBACKDROP_TYPE`: Windows 11 22H2.
    const SYSTEM_BACKDROP_BUILD: u32 = 22621;

    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    /// `DWMSBT_AUTO`: let DWM decide, which for a window like ours draws no backdrop.
    const DWMSBT_AUTO: u32 = 0;
    /// `DWMSBT_TRANSIENTWINDOW`: the acrylic backdrop.
    const DWMSBT_TRANSIENTWINDOW: u32 = 3;

    #[repr(C)]
    struct Margins {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct OsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: isize,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
        fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const Margins) -> i32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
    }

    pub(super) fn has_system_backdrop() -> bool {
        static SUPPORTED: OnceLock<bool> = OnceLock::new();
        *SUPPORTED.get_or_init(|| {
            let mut info = OsVersionInfo {
                size: size_of::<OsVersionInfo>() as u32,
                major: 0,
                minor: 0,
                build: 0,
                platform: 0,
                service_pack: [0; 128],
            };
            // `RtlGetVersion` reports the real build, unlike `GetVersionEx` without a manifest.
            unsafe { RtlGetVersion(&mut info) == 0 && info.build >= SYSTEM_BACKDROP_BUILD }
        })
    }

    fn set_attribute(hwnd: isize, attribute: u32, value: u32) {
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute,
                (&raw const value).cast(),
                size_of::<u32>() as u32,
            );
        }
    }

    /// Put DWM's acrylic system backdrop behind a blurred window, or take it away, and return the
    /// background gpui should still be asked for.
    ///
    /// With the system backdrop in place gpui is asked for a transparent window, so it clears each
    /// frame to nothing and the backdrop shows through, and the caller then turns gpui's accent
    /// policy off with [`disable_accent`], which the backdrop needs. An opaque window would keep
    /// the accent off by itself, but gpui-pre clears an opaque window to white, and the white
    /// showed through the translucent chrome instead of the backdrop. `dark` picks the
    /// backdrop's tint, to match the theme rather than the system. The flag says whether the
    /// accent has to be turned off.
    pub(super) fn apply_system_backdrop(
        window: &Window,
        background: WindowBackgroundAppearance,
        dark: bool,
    ) -> (WindowBackgroundAppearance, bool) {
        if !has_system_backdrop() {
            return (background, false);
        }
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return (background, false);
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return (background, false);
        };
        let hwnd = handle.hwnd.get();
        let blurred = background == WindowBackgroundAppearance::Blurred;

        // A margin of -1 extends the frame over the whole client area, so the backdrop the frame
        // draws reaches behind all of it.
        let inset = if blurred { -1 } else { 0 };
        let margins = Margins {
            left: inset,
            right: inset,
            top: inset,
            bottom: inset,
        };
        unsafe {
            DwmExtendFrameIntoClientArea(hwnd, &margins);
        }
        if blurred {
            set_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, u32::from(dark));
        }
        set_attribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            if blurred {
                DWMSBT_TRANSIENTWINDOW
            } else {
                DWMSBT_AUTO
            },
        );

        if blurred {
            (WindowBackgroundAppearance::Transparent, true)
        } else {
            (background, false)
        }
    }

    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient: u32,
        animation: u32,
    }

    #[repr(C)]
    struct CompositionAttribute {
        attribute: u32,
        data: *mut c_void,
        size: usize,
    }

    /// `WCA_ACCENT_POLICY`.
    const ACCENT_POLICY: u32 = 0x13;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleA(name: *const u8) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    }

    /// Turn the window's accent policy off (`ACCENT_DISABLED`), as gpui does for an opaque
    /// window.
    ///
    /// gpui gives a transparent window the transparent-gradient accent, which would paint over
    /// the system backdrop. `SetWindowCompositionAttribute` is undocumented and not in the import
    /// library, so it is looked up in `user32.dll` the way gpui itself does.
    pub(super) fn disable_accent(window: &Window) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        type SetWindowCompositionAttribute =
            unsafe extern "system" fn(isize, *mut CompositionAttribute) -> i32;
        unsafe {
            let user32 = GetModuleHandleA(c"user32.dll".as_ptr().cast());
            if user32 == 0 {
                return;
            }
            let function = GetProcAddress(user32, c"SetWindowCompositionAttribute".as_ptr().cast());
            if function.is_null() {
                return;
            }
            let set: SetWindowCompositionAttribute = std::mem::transmute(function);
            let mut accent = AccentPolicy {
                state: 0,
                flags: 0,
                gradient: 0,
                animation: 0,
            };
            let mut data = CompositionAttribute {
                attribute: ACCENT_POLICY,
                data: (&raw mut accent).cast(),
                size: size_of::<AccentPolicy>(),
            };
            set(handle.hwnd.get(), &mut data);
        }
    }
}
