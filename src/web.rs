//! The design language on the web: the tokens as CSS custom properties, for a page in a browser
//! or a Tauri webview that should look like an Atelier app.
//!
//! The web kit under `web/atelier/` styles its components only through these properties, so the
//! rules that derive a palette stay here, in [`Theme::new`], and the web follows them. The files
//! it ships are written from this module; `web_files_are_current` fails when they fall behind,
//! and `ATELIER_BLESS_WEB=1 cargo test --lib web` writes them again.
//!
//! A Tauri app can also call [`theme_css`] itself, to hand its webview a theme the six shipped
//! ones don't cover, such as a zoomed one.

use std::fmt::Write as _;

use gpui::{BoxShadow, FontWeight, Hsla, Rgba};

use crate::button::{ButtonVariant, paint};
use crate::theme::{Direction, Material, PrimaryStyle, Theme, mix};

/// The interface font on the web: each platform's own, then Japanese, then emoji.
const UI_FONT: &str = "system-ui, -apple-system, \"Segoe UI\", Inter, \"Hiragino Sans\", \
                       \"Yu Gothic UI\", Meiryo, \"Noto Sans CJK JP\", \"Apple Color Emoji\", \
                       \"Segoe UI Emoji\", sans-serif";
/// The monospaced font on the web.
const MONO_FONT: &str = "ui-monospace, Menlo, \"Cascadia Mono\", Consolas, \
                         \"DejaVu Sans Mono\", \"Hiragino Sans\", monospace";

/// How far translucent chrome blurs what lies behind it, in pixels.
const CHROME_BLUR: f32 = 28.;

/// The identifier a page selects `theme` by: `data-theme="glass-dark"`.
pub fn theme_id(theme: &Theme) -> String {
    format!(
        "{}-{}",
        theme.direction.id(),
        theme.appearance.label().to_lowercase()
    )
}

/// The stylesheet of every shipped theme: Glass Dark, the default, on `:root`, then each theme
/// under `[data-theme="…"]`.
pub fn stylesheet() -> String {
    let mut css = String::from(
        "/* Atelier design tokens, written by `atelier::web::stylesheet`. Do not edit: change\n   \
         src/theme.rs and run `ATELIER_BLESS_WEB=1 cargo test --lib web`. */\n",
    );
    // The default comes first, so a `data-theme` anywhere overrides it whatever the order.
    let _ = write!(css, "\n:root {{\n{}}}\n", theme_css(&Theme::default()));
    for theme in Theme::all() {
        let _ = write!(
            css,
            "\n[data-theme=\"{}\"] {{\n{}}}\n",
            theme_id(&theme),
            theme_css(&theme)
        );
    }
    css
}

/// `theme`'s tokens as CSS custom property declarations, one per line, ready to go inside a
/// rule.
pub fn theme_css(theme: &Theme) -> String {
    let mut css = String::new();
    let mut put = |name: &str, value: String| {
        let _ = writeln!(css, "  --at-{name}: {value};");
    };
    let colors = &theme.colors;
    let dark = theme.appearance.is_dark();

    put("color-scheme", if dark { "dark" } else { "light" }.into());
    put(
        "translucent",
        u8::from(theme.material == Material::Translucent).to_string(),
    );
    put(
        "chrome-blur",
        match theme.material {
            Material::Translucent => pixels(CHROME_BLUR),
            Material::Opaque => "0px".into(),
        },
    );

    for (name, color) in [
        ("background", colors.background),
        ("chrome", colors.chrome),
        ("surface", colors.surface),
        ("elevated", colors.elevated),
        ("overlay", colors.overlay),
        ("sunken", colors.sunken),
        ("hover", colors.hover),
        ("pressed", colors.pressed),
        ("selected", colors.selected),
        ("border", colors.border),
        ("border-strong", colors.border_strong),
        ("separator", colors.separator),
        ("highlight", colors.highlight),
        ("focus-ring", colors.focus_ring),
        ("selection", colors.selection),
        ("text", colors.text),
        ("text-muted", colors.text_muted),
        ("text-faint", colors.text_faint),
        ("text-disabled", colors.text_disabled),
        ("accent", colors.accent),
        ("accent-hover", colors.accent_hover),
        ("accent-text", colors.accent_text),
        ("accent-wash", colors.accent_wash),
        ("on-accent", colors.on_accent),
        ("solid", colors.solid),
        ("on-solid", colors.on_solid),
        ("success", colors.success),
        ("warning", colors.warning),
        ("danger", colors.danger),
        ("info", colors.info),
        ("success-wash", colors.success_wash),
        ("warning-wash", colors.warning_wash),
        ("danger-wash", colors.danger_wash),
        ("info-wash", colors.info_wash),
        ("agent", colors.agent),
        ("agent-wash", colors.agent_wash),
        ("user-bubble", colors.user_bubble),
        ("on-user-bubble", colors.on_user_bubble),
        ("record", colors.record),
        ("playhead", colors.playhead),
        ("grid", colors.grid),
        ("grid-faint", colors.grid_faint),
    ] {
        put(name, color_css(color));
    }
    for (name, ramp) in [
        ("agent-gradient", &colors.agent_gradient[..]),
        ("meter", &colors.meter[..]),
        ("series", &colors.series[..]),
        ("heat", &colors.heat[..]),
        ("port", &colors.ports[..]),
        ("metal", &colors.metal[..]),
    ] {
        for (index, &color) in ramp.iter().enumerate() {
            put(&format!("{name}-{index}"), color_css(color));
        }
    }
    for index in 0..8 {
        put(
            &format!("category-{index}"),
            color_css(theme.categorical(index)),
        );
    }
    // The fill a switch or slider takes when on, as `controls::on_fill` picks it.
    put(
        "on-fill",
        color_css(match theme.primary_style {
            PrimaryStyle::Accent => colors.accent,
            PrimaryStyle::Solid => colors.solid,
        }),
    );
    put(
        "on-fill-thumb",
        color_css(match theme.primary_style {
            PrimaryStyle::Accent => gpui::white(),
            PrimaryStyle::Solid => colors.on_solid,
        }),
    );

    // Buttons, painted by the same rules as `Button`.
    for (name, variant) in [
        ("primary", ButtonVariant::Primary),
        ("secondary", ButtonVariant::Secondary),
        ("ghost", ButtonVariant::Ghost),
        ("danger", ButtonVariant::Danger),
        ("agent", ButtonVariant::Agent),
    ] {
        let paint = paint(theme, variant, false, None);
        put(&format!("button-{name}-fill"), color_css(paint.fill));
        put(&format!("button-{name}-hover"), color_css(paint.hover));
        put(
            &format!("button-{name}-pressed"),
            color_css(mix(paint.hover, paint.text, 0.08)),
        );
        put(&format!("button-{name}-text"), color_css(paint.text));
        put(
            &format!("button-{name}-border"),
            paint.border.map_or("transparent".into(), color_css),
        );
    }
    let selected = paint(theme, ButtonVariant::Secondary, true, None);
    put("button-selected-fill", color_css(selected.fill));
    put("button-selected-hover", color_css(selected.hover));
    put("button-selected-text", color_css(selected.text));
    // The chosen segment of a segmented control, as `SegmentedControl` fills it.
    put(
        "segment-selected",
        color_css(mix(
            colors.elevated,
            colors.text,
            if dark { 0.08 } else { 0. },
        )),
    );
    // Raised controls catch the light from above where the direction has a bezel.
    let bezel = colors.highlight.a > 0.;
    let raised = paint(theme, ButtonVariant::Secondary, false, None).fill;
    put(
        "raised-top",
        color_css(if bezel {
            mix(raised, gpui::white(), if dark { 0.05 } else { 0.5 })
        } else {
            raised
        }),
    );
    put(
        "raised-shadow",
        if bezel {
            shadows_css(&theme.elevation.low)
        } else {
            "none".into()
        },
    );

    let shape = &theme.shape;
    for (name, radius) in [
        ("radius-xs", shape.radius_xs),
        ("radius-sm", shape.radius_sm),
        ("radius-md", shape.radius_md),
        ("radius-lg", shape.radius_lg),
        ("radius-xl", shape.radius_xl),
    ] {
        put(name, pixels(f32::from(radius)));
    }
    let density = &theme.density;
    for (name, size) in [
        ("control-sm", density.control_sm),
        ("control-md", density.control_md),
        ("control-lg", density.control_lg),
    ] {
        put(name, pixels(f32::from(size)));
        put(
            &format!("{name}-radius"),
            pixels(f32::from(theme.control_radius(size))),
        );
    }
    for (name, size) in [
        ("row", density.row),
        ("control-padding", density.control_padding),
        ("panel-padding", density.panel_padding),
        ("toolbar", density.toolbar),
        ("title-bar", density.title_bar),
        ("status-bar", density.status_bar),
        ("form-width", density.form_width),
        ("split-width", density.split_width),
        ("sidebar-width", density.sidebar_width),
    ] {
        put(name, pixels(f32::from(size)));
    }
    let spacing = &theme.spacing;
    for (name, size) in [
        ("space-xxs", spacing.xxs),
        ("space-xs", spacing.xs),
        ("space-sm", spacing.sm),
        ("space-md", spacing.md),
        ("space-lg", spacing.lg),
        ("space-xl", spacing.xl),
        ("space-xxl", spacing.xxl),
    ] {
        put(name, pixels(f32::from(size)));
    }

    let typography = &theme.typography;
    put("font-ui", UI_FONT.into());
    put("font-mono", MONO_FONT.into());
    for (name, size) in [
        ("size-xs", typography.size_xs),
        ("size-sm", typography.size_sm),
        ("size-md", typography.size_md),
        ("size-lg", typography.size_lg),
        ("size-xl", typography.size_xl),
        ("size-display", typography.size_display),
    ] {
        put(name, pixels(f32::from(size)));
    }
    put("heading-weight", weight(typography.heading_weight));
    put("label-weight", weight(typography.label_weight));

    let elevation = &theme.elevation;
    put("shadow-low", shadows_css(&elevation.low));
    put("shadow-medium", shadows_css(&elevation.medium));
    put("shadow-high", shadows_css(&elevation.high));

    let motion = &theme.motion;
    put("motion-fast", format!("{}ms", motion.fast.as_millis()));
    put("motion-normal", format!("{}ms", motion.normal.as_millis()));
    put("motion-slow", format!("{}ms", motion.slow.as_millis()));
    // Mono keeps a hairline for its separators; the others a plain one.
    put(
        "hairline",
        if theme.direction == Direction::Mono {
            "0.5px"
        } else {
            "1px"
        }
        .into(),
    );
    css
}

#[cfg(test)]
fn js_string(text: &str) -> String {
    format!(
        "`{}`",
        text.replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${")
    )
}

fn color_css(color: Hsla) -> String {
    let Rgba { r, g, b, a } = color.into();
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    let (r, g, b) = (channel(r), channel(g), channel(b));
    if a >= 0.999 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("rgb({r} {g} {b} / {})", trim(a))
    }
}

fn shadows_css(shadows: &[BoxShadow]) -> String {
    shadows
        .iter()
        .map(|shadow| {
            format!(
                "{} {} {} {} {}",
                pixels(f32::from(shadow.offset.x)),
                pixels(f32::from(shadow.offset.y)),
                pixels(f32::from(shadow.blur_radius)),
                pixels(f32::from(shadow.spread_radius)),
                color_css(shadow.color)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn pixels(value: f32) -> String {
    format!("{}px", trim(value))
}

fn weight(weight: FontWeight) -> String {
    trim(weight.0)
}

/// A number with at most three decimals and no trailing zeros.
fn trim(value: f32) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The JavaScript module the web kit's ambient backdrops run on: the shaders, as the native
    /// backdrops run them, how their parameters are laid out, and each theme's palette.
    fn ambient_module() -> String {
        let mut js = String::from(
            "// Atelier ambient backdrops, written by `atelier::web`. Do not edit: change\n\
             // src/ambient and run `ATELIER_BLESS_WEB=1 cargo test --lib web`.\n\n",
        );
        let layout = crate::ambient::web_layout();
        let _ = writeln!(js, "export const layout = {{");
        for (name, value) in layout {
            let _ = writeln!(js, "  {name}: {value},");
        }
        let _ = writeln!(js, "}};\n");

        let _ = writeln!(js, "export const palettes = {{");
        for theme in Theme::all() {
            let [base, deep, hues @ ..] = crate::ambient::web_palette(&theme);
            let triple = |[r, g, b]: [f32; 3]| format!("[{r:.4}, {g:.4}, {b:.4}]");
            let _ = writeln!(
                js,
                "  \"{}\": {{ dark: {}, base: {}, deep: {}, hues: [{}] }},",
                theme_id(&theme),
                theme.appearance.is_dark(),
                triple(base),
                triple(deep),
                hues.map(triple).join(", "),
            );
        }
        let _ = writeln!(js, "}};\n");

        let _ = writeln!(
            js,
            "export const common = {};\n",
            js_string(crate::ambient::web_common())
        );
        let _ = writeln!(js, "export const scenes = {{");
        for (ambience, source, period) in crate::ambient::web_scenes() {
            let _ = writeln!(
                js,
                "  {}: {{ period: {period}, shader: {} }},",
                ambience.label(),
                js_string(source)
            );
        }
        let _ = writeln!(js, "}};");
        js
    }

    /// The JavaScript module of the icons, as inline SVG, each drawn in `currentColor`.
    fn icons_module() -> String {
        let mut js = String::from(
            "// Atelier icons, written by `atelier::web` from assets/icons. Do not edit.\n\n\
             export const icons = {\n",
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
        for (dir, prefix) in [("icons", ""), ("logos", "logo-")] {
            for entry in std::fs::read_dir(root.join(dir)).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|extension| extension == "svg") {
                    let stem = path.file_stem().unwrap().to_string_lossy();
                    files.push((format!("{prefix}{stem}"), path));
                }
            }
        }
        files.sort();
        for (name, path) in files {
            let svg = std::fs::read_to_string(path).unwrap();
            let svg = svg
                .trim()
                .replace("\"black\"", "\"currentColor\"")
                .replace("#000000", "currentColor")
                .replace("#000", "currentColor");
            let _ = writeln!(js, "  \"{name}\": {},", js_string(&svg));
        }
        js.push_str("};\n");
        js
    }

    /// The files the web kit ships, and what this module writes into them.
    fn files() -> [(&'static str, String); 3] {
        [
            ("web/atelier/generated/tokens.css", stylesheet()),
            ("web/atelier/generated/ambient.js", ambient_module()),
            ("web/atelier/generated/icons.js", icons_module()),
        ]
    }

    #[test]
    fn web_files_are_current() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let bless = std::env::var_os("ATELIER_BLESS_WEB").is_some();
        for (path, contents) in files() {
            let path = root.join(path);
            if bless {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, contents).unwrap();
                continue;
            }
            let shipped = std::fs::read_to_string(&path).unwrap_or_default();
            assert!(
                shipped == contents,
                "{} is out of date: run `ATELIER_BLESS_WEB=1 cargo test --lib web`",
                path.display()
            );
        }
    }

    #[test]
    fn every_theme_has_its_own_rule() {
        let css = stylesheet();
        for theme in Theme::all() {
            assert!(css.contains(&format!("[data-theme=\"{}\"]", theme_id(&theme))));
        }
        assert!(css.contains(":root"));
    }

    #[test]
    fn colours_are_written_as_css() {
        assert_eq!(color_css(gpui::rgb(0x0a84ff).into()), "#0a84ff");
        assert_eq!(
            color_css(gpui::white().opacity(0.5)),
            "rgb(255 255 255 / 0.5)"
        );
        assert_eq!(trim(12.5), "12.5");
        assert_eq!(trim(9.), "9");
    }
}
