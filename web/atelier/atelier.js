// Atelier for the web: what the CSS can't do alone. Picking a theme, drawing icons, and the
// small behaviours of segmented controls, tabs, sliders and knobs. No framework: call `hydrate` on any
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
/// segmented controls and tabs select on click and with the arrow keys, keep slider fills in
/// step with their values, and draw and turn knobs. Safe to call again on the same subtree.
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
    const bipolar = slider.classList.contains("at-slider--bipolar");
    const fill = () => {
      const span = slider.max - slider.min || 1;
      const at = ((slider.value - slider.min) / span) * 100;
      slider.style.setProperty("--_value", `${bipolar ? Math.max(at, 50) : at}%`);
      if (bipolar) slider.style.setProperty("--_from", `${Math.min(at, 50)}%`);
    };
    fill();
    if (!slider.dataset.atReady) {
      slider.dataset.atReady = "1";
      slider.addEventListener("input", fill);
      // A double click goes back to `data-default`, or the centre for a bipolar slider.
      slider.addEventListener("dblclick", () => {
        const middle = (Number(slider.min || 0) + Number(slider.max || 100)) / 2;
        const reset = slider.dataset.default ?? (bipolar ? middle : null);
        if (reset == null) return;
        slider.value = reset;
        slider.dispatchEvent(new Event("input", { bubbles: true }));
        slider.dispatchEvent(new Event("change", { bubbles: true }));
      });
    }
  }
  for (const knob of root.querySelectorAll(".at-knob")) knobify(knob);
}

/// The knob's sweep: from 7:30 clockwise to 4:30, in radians from +x, y pointing down.
const SWEEP_START = 0.75 * Math.PI;
const SWEEP = 1.5 * Math.PI;
/// Pixels of drag that turn a knob through its whole range.
const TURN_TRAVEL = 200;

/// An SVG arc around (20, 20) from angle `from` to `to`.
function arc(radius, from, to) {
  if (Math.abs(to - from) < 0.001) return "";
  const at = (angle) => `${20 + radius * Math.cos(angle)} ${20 + radius * Math.sin(angle)}`;
  const large = Math.abs(to - from) > Math.PI ? 1 : 0;
  return `M ${at(from)} A ${radius} ${radius} 0 ${large} 1 ${at(to)}`;
}

/// Make a `.at-knob` turn. Its value, from 0 to 1, is `data-value`; `data-bipolar` draws the arc
/// from twelve o'clock, `data-default` is where a double click returns it (the centre for a
/// bipolar knob), `data-step` is one arrow key's worth (0.01), and `--_size` its diameter.
/// Turning it sets `knob.value` and `data-value` and fires `input` on the knob, then `change`
/// when the drag ends; set `knob.value` from script to move it.
function knobify(knob) {
  if (knob.dataset.atReady) return;
  knob.dataset.atReady = "1";
  let dial = knob.querySelector(".at-knob__dial");
  if (!dial) {
    dial = document.createElement("span");
    dial.className = "at-knob__dial";
    knob.prepend(dial);
  }
  const disabled = knob.getAttribute("aria-disabled") === "true";
  dial.setAttribute("role", "slider");
  dial.setAttribute("aria-valuemin", "0");
  dial.setAttribute("aria-valuemax", "100");
  const label = knob.querySelector(".at-knob__label");
  if (label && !dial.hasAttribute("aria-label")) dial.setAttribute("aria-label", label.textContent.trim());
  if (!disabled) dial.tabIndex = 0;
  const radius = 17;
  const body = radius - 6;
  dial.innerHTML = `<svg viewBox="0 0 40 40" aria-hidden="true">
    <path class="at-knob__track" d="${arc(radius, SWEEP_START, SWEEP_START + SWEEP)}"/>
    <path class="at-knob__ring" d="${arc(radius + 2, SWEEP_START, SWEEP_START + SWEEP)}"/>
    <path class="at-knob__value"/>
    <circle class="at-knob__body" cx="20" cy="20" r="${body}"/>
    <line class="at-knob__pointer"/>
  </svg>`;
  const valuePath = dial.querySelector(".at-knob__value");
  const pointer = dial.querySelector(".at-knob__pointer");
  const bipolar = knob.hasAttribute("data-bipolar");
  const step = Number(knob.dataset.step) || 0.01;
  let value = Math.min(1, Math.max(0, Number(knob.dataset.value) || 0));

  const draw = () => {
    const at = SWEEP_START + SWEEP * value;
    const origin = bipolar ? SWEEP_START + SWEEP / 2 : SWEEP_START;
    valuePath.setAttribute("d", arc(radius, Math.min(at, origin), Math.max(at, origin)));
    pointer.setAttribute("x1", 20 + body * 0.25 * Math.cos(at));
    pointer.setAttribute("y1", 20 + body * 0.25 * Math.sin(at));
    pointer.setAttribute("x2", 20 + body * 0.8 * Math.cos(at));
    pointer.setAttribute("y2", 20 + body * 0.8 * Math.sin(at));
    dial.setAttribute("aria-valuenow", String(Math.round(value * 100)));
    knob.dataset.value = String(value);
  };
  const set = (next, type = "input") => {
    next = Math.min(1, Math.max(0, next));
    if (next !== value) {
      value = next;
      draw();
      knob.dispatchEvent(new Event("input", { bubbles: true }));
    }
    if (type === "change") knob.dispatchEvent(new Event("change", { bubbles: true }));
  };
  Object.defineProperty(knob, "value", {
    get: () => value,
    set: (next) => {
      value = Math.min(1, Math.max(0, Number(next) || 0));
      draw();
    },
  });
  draw();
  if (disabled) return;

  dial.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    const from = { x: event.clientX, y: event.clientY, value };
    dial.setPointerCapture(event.pointerId);
    dial.focus({ preventScroll: true });
    const move = (event) => set(from.value + (event.clientX - from.x - (event.clientY - from.y)) / TURN_TRAVEL);
    const up = () => {
      dial.removeEventListener("pointermove", move);
      dial.removeEventListener("pointerup", up);
      dial.removeEventListener("pointercancel", up);
      if (value !== from.value) set(value, "change");
    };
    dial.addEventListener("pointermove", move);
    dial.addEventListener("pointerup", up);
    dial.addEventListener("pointercancel", up);
    event.preventDefault();
  });
  dial.addEventListener("dblclick", () => {
    const reset = knob.dataset.default ?? (bipolar ? 0.5 : null);
    if (reset != null) set(Number(reset), "change");
  });
  dial.addEventListener("keydown", (event) => {
    const next = {
      ArrowUp: value + step,
      ArrowRight: value + step,
      ArrowDown: value - step,
      ArrowLeft: value - step,
      Home: 0,
      End: 1,
    }[event.key];
    if (next === undefined) return;
    set(next, "change");
    event.preventDefault();
  });
}

/// Tauri: make the window's chrome translucent over the desktop where the theme is, by telling
/// the page the window has vibrancy (see web/README.md for the window configuration).
export function useVibrancy(enabled = true) {
  if (enabled) document.documentElement.dataset.vibrancy = "";
  else delete document.documentElement.dataset.vibrancy;
}
