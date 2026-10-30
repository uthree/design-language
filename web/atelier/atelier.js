// Atelier for the web: what the CSS can't do alone. Picking a theme, drawing icons, and the
// small behaviours of segmented controls, tabs and sliders. No framework: call `hydrate` on any
// subtree a framework renders, or use the CSS alone and wire behaviour yourself.

import { icons } from "./generated/icons.js";

export { icons };

export const DIRECTIONS = ["studio", "glass", "mono"];
export const APPEARANCES = ["light", "dark"];

/// The theme in force, as `{ direction, appearance }`.
export function theme() {
  const [direction, appearance] = (document.documentElement.dataset.theme || "glass-dark").split("-");
  return { direction, appearance };
}

/// Install a theme on the page: `setTheme({ direction: "studio" })` keeps the appearance.
/// Fires `atelier:theme` on the document.
export function setTheme({ direction, appearance } = {}) {
  const current = theme();
  const id = `${direction || current.direction}-${appearance || current.appearance}`;
  document.documentElement.dataset.theme = id;
  document.dispatchEvent(new CustomEvent("atelier:theme", { detail: id }));
  return id;
}

/// Follow the system's light or dark preference until `setTheme` names an appearance.
export function followSystemAppearance() {
  const query = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => setTheme({ appearance: query.matches ? "dark" : "light" });
  apply();
  query.addEventListener("change", apply);
  return () => query.removeEventListener("change", apply);
}

/// An icon element, drawn in the current text colour.
export function icon(name, { size } = {}) {
  const span = document.createElement("span");
  span.className = "at-icon";
  span.setAttribute("aria-hidden", "true");
  span.innerHTML = icons[name] || "";
  if (size) {
    span.style.width = `${size}px`;
    span.style.height = `${size}px`;
  }
  return span;
}

/// Bring the markup under `root` to life: fill `[data-icon]` elements with their icon, make
/// segmented controls and tabs select on click and with the arrow keys, and keep slider fills
/// in step with their values. Safe to call again on the same subtree.
export function hydrate(root = document) {
  for (const element of root.querySelectorAll("[data-icon]")) {
    if (element.dataset.iconDrawn === element.dataset.icon) continue;
    element.classList.add("at-icon");
    element.setAttribute("aria-hidden", "true");
    element.innerHTML = icons[element.dataset.icon] || "";
    element.dataset.iconDrawn = element.dataset.icon;
  }
  for (const group of root.querySelectorAll(".at-segmented, .at-tabs")) {
    if (group.dataset.atReady) continue;
    group.dataset.atReady = "1";
    const attribute = group.classList.contains("at-tabs") ? "aria-selected" : "aria-pressed";
    const items = () => [...group.querySelectorAll(":scope > button")];
    const select = (chosen) => {
      for (const item of items()) item.setAttribute(attribute, String(item === chosen));
      group.dispatchEvent(
        new CustomEvent("atelier:select", {
          bubbles: true,
          detail: { index: items().indexOf(chosen), value: chosen.value || chosen.textContent.trim() },
        }),
      );
    };
    group.addEventListener("click", (event) => {
      const item = event.target.closest("button");
      if (item && item.parentElement === group) select(item);
    });
    group.addEventListener("keydown", (event) => {
      const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
      if (!step) return;
      const list = items();
      const next = list[(list.indexOf(document.activeElement) + step + list.length) % list.length];
      next.focus();
      select(next);
      event.preventDefault();
    });
  }
  for (const slider of root.querySelectorAll("input.at-slider")) {
    const fill = () => {
      const span = slider.max - slider.min || 1;
      slider.style.setProperty("--_value", `${((slider.value - slider.min) / span) * 100}%`);
    };
    fill();
    if (!slider.dataset.atReady) {
      slider.dataset.atReady = "1";
      slider.addEventListener("input", fill);
    }
  }
}

/// Tauri: make the window's chrome translucent over the desktop where the theme is, by telling
/// the page the window has vibrancy (see web/README.md for the window configuration).
export function useVibrancy(enabled = true) {
  if (enabled) document.documentElement.dataset.vibrancy = "";
  else delete document.documentElement.dataset.vibrancy;
}
