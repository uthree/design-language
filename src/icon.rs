//! The icon set, and the asset source that serves it.
//!
//! Icons are 24×24 SVGs drawn on a 2px stroke, embedded in the binary so an application needs no
//! asset directory. gpui renders an SVG as a mask, so an icon takes the colour it is given and
//! nothing in the file decides it.

use std::borrow::Cow;

use gpui::{
    App, AssetSource, Hsla, IntoElement, Pixels, RenderOnce, SharedString, Styled, Window, img, px,
    svg,
};

use crate::theme::ActiveTheme;

macro_rules! icons {
    ($($variant:ident => $file:literal),* $(,)?) => {
        /// Every icon in the set.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum IconName {
            $(
                #[doc = concat!("`", $file, "`")]
                $variant,
            )*
        }

        impl IconName {
            /// Every icon, in declaration order.
            pub const ALL: &[Self] = &[$(Self::$variant),*];

            /// Asset path served by [`Assets`].
            pub fn path(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("icons/", $file, ".svg"),)*
                }
            }

            /// The file stem, for galleries and debugging.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $file,)*
                }
            }
        }

        fn embedded(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $file, ".svg") => {
                    Some(include_bytes!(concat!("../assets/icons/", $file, ".svg")))
                })*
                _ => None,
            }
        }
    };
}

icons! {
    Alert => "alert",
    ArrowUp => "arrow-up",
    Bolt => "bolt",
    Brain => "brain",
    Check => "check",
    ChevronDown => "chevron-down",
    ChevronLeft => "chevron-left",
    ChevronRight => "chevron-right",
    ChevronUp => "chevron-up",
    Close => "close",
    Code => "code",
    Copy => "copy",
    Dot => "dot",
    Eye => "eye",
    File => "file",
    Folder => "folder",
    Headphones => "headphones",
    History => "history",
    Info => "info",
    Keyboard => "keyboard",
    Knob => "knob",
    Loop => "loop",
    Metronome => "metronome",
    Mic => "mic",
    Minus => "minus",
    More => "more",
    Music => "music",
    Paperclip => "paperclip",
    Pause => "pause",
    Play => "play",
    Plus => "plus",
    Record => "record",
    Redo => "redo",
    Refresh => "refresh",
    Search => "search",
    Settings => "settings",
    SidebarLeft => "sidebar-left",
    SidebarRight => "sidebar-right",
    SkipBack => "skip-back",
    SkipForward => "skip-forward",
    Sliders => "sliders",
    Sparkles => "sparkles",
    Spinner => "spinner",
    Stop => "stop",
    Trash => "trash",
    Undo => "undo",
    User => "user",
    Volume => "volume",
    VolumeOff => "volume-off",
    Waveform => "waveform",
    WindowClose => "window-close",
    WindowMaximize => "window-maximize",
    WindowMinimize => "window-minimize",
    WindowRestore => "window-restore",
    Wrench => "wrench",
}

/// Serves the embedded icons, then defers to an application's own source for anything else.
///
/// ```ignore
/// Application::new().with_assets(atelier::Assets::default())
/// // or, with assets of your own:
/// Application::new().with_assets(atelier::Assets::with_fallback(MyAssets))
/// ```
#[derive(Default)]
pub struct Assets {
    fallback: Option<Box<dyn AssetSource>>,
}

impl Assets {
    /// Serve the icon set first and `fallback` for every other path.
    pub fn with_fallback(fallback: impl AssetSource) -> Self {
        Self {
            fallback: Some(Box::new(fallback)),
        }
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = embedded(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        match &self.fallback {
            Some(fallback) => fallback.load(path),
            None => Ok(None),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut found: Vec<SharedString> = IconName::ALL
            .iter()
            .map(|icon| icon.path())
            .filter(|candidate| candidate.starts_with(path))
            .map(SharedString::from)
            .collect();
        if let Some(fallback) = &self.fallback {
            found.extend(fallback.list(path)?);
        }
        Ok(found)
    }
}

/// What an icon draws: one of the set, an application's own SVG, or a full-colour image.
///
/// Everything that takes an icon takes `impl Into<IconSource>`, so an [`IconName`] still works
/// as before and an application's own glyph goes in the same place:
///
/// ```ignore
/// ListItem::new("tempo", "Tempo").icon(IconName::Metronome)
/// ListItem::new("punch", "Punch in").icon(IconSource::svg("icons/auris-punch.svg"))
/// OptionCard::new("openai", IconSource::image("logos/provider.png"), "Provider")
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconSource {
    /// An icon from the set.
    Named(IconName),
    /// A single-colour SVG served by the application's asset source (see
    /// [`Assets::with_fallback`]). It is drawn as a mask in the colour it is given, like the set,
    /// so draw it on the same 24×24 grid and 2px stroke.
    Svg(SharedString),
    /// A full-colour image served by the asset source, drawn as it is: a provider's logo, an
    /// avatar. It takes no colour.
    Image(SharedString),
}

impl IconSource {
    /// An application SVG drawn like the set.
    pub fn svg(path: impl Into<SharedString>) -> Self {
        Self::Svg(path.into())
    }

    /// A full-colour image.
    pub fn image(path: impl Into<SharedString>) -> Self {
        Self::Image(path.into())
    }
}

impl From<IconName> for IconSource {
    fn from(name: IconName) -> Self {
        Self::Named(name)
    }
}

/// An icon element. Defaults to the muted text colour at 16px.
#[derive(IntoElement)]
pub struct Icon {
    source: IconSource,
    size: Option<Pixels>,
    color: Option<Hsla>,
}

impl Icon {
    /// The icon drawing `source`: an [`IconName`] or an [`IconSource`].
    pub fn new(source: impl Into<IconSource>) -> Self {
        Self {
            source: source.into(),
            size: None,
            color: None,
        }
    }

    /// Edge length in pixels.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = Some(size);
        self
    }

    /// Colour; defaults to [`crate::Colors::text_muted`]. An [`IconSource::Image`] ignores it.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self {
        Self::new(name)
    }
}

impl From<IconSource> for Icon {
    fn from(source: IconSource) -> Self {
        Self::new(source)
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let size = self.size.unwrap_or(px(16.));
        let path: SharedString = match self.source {
            IconSource::Named(name) => name.path().into(),
            IconSource::Svg(path) => path,
            IconSource::Image(path) => {
                return img(path).flex_none().size(size).into_any_element();
            }
        };
        svg()
            .path(path)
            .flex_none()
            .size(size)
            .text_color(self.color.unwrap_or(cx.theme().colors.text_muted))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_embedded() {
        let assets = Assets::default();
        for icon in IconName::ALL {
            let bytes = assets.load(icon.path()).unwrap().unwrap();
            assert!(bytes.starts_with(b"<svg"), "{}", icon.name());
        }
    }
}
