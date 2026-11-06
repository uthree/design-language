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
  for (const plan of root.querySelectorAll(".at-plan")) foldablePlan(plan);
}

/// A plan's header folds it to the step in progress, marked `.at-step--current`.
function foldablePlan(plan) {
  const steps = [...plan.querySelectorAll(".at-step")];
  for (const step of steps) step.classList.remove("at-step--current");
  const current =
    steps.find((step) => step.matches(".at-step--running, .at-step--failed")) ||
    steps.find((step) => !step.matches(".at-step--done, .at-step--skipped"));
  current?.classList.add("at-step--current");
  const header = plan.querySelector(".at-plan__header");
  if (!header || header.dataset.atReady) return;
  header.dataset.atReady = "1";
  header.setAttribute("aria-expanded", String(!plan.classList.contains("at-plan--closed")));
  header.addEventListener("click", () => {
    const open = plan.classList.toggle("at-plan--closed") === false;
    header.setAttribute("aria-expanded", String(open));
    plan.dispatchEvent(new CustomEvent("atelier:toggle", { bubbles: true, detail: { open } }));
  });
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

// ---------------------------------------------------------------- the conversation view

/// The words a chat view draws itself, in English; pass `strings` to `chat()` to translate.
export const CHAT_STRINGS = {
  assistant: "Assistant",
  allow: "Allow",
  deny: "Deny",
  alwaysAllow: "Always allow",
  answers: { allow: "Allowed", alwaysAllow: "Always allowed", deny: "Denied" },
  jumpToLatest: "Jump to latest",
  newMessages: (count) => `${count} new`,
  usedTools: (count) => `Used ${count} tools`,
  copy: "Copy",
};

/// How close to the end, in pixels, still counts as reading the latest entry.
const TAIL_SLACK = 64;

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

/// The little Markdown a chat answer needs, as `ChatView` draws it: paragraphs, `-` bullets,
/// `#` headings and fenced code. Text is never parsed as HTML.
function prose(target, text) {
  target.replaceChildren();
  const lines = text.split("\n");
  let paragraph = [];
  let list = null;
  const flush = () => {
    if (paragraph.length) target.append(element("p", null, paragraph.join(" ")));
    paragraph = [];
  };
  for (let index = 0; index < lines.length; index++) {
    const line = lines[index].trimStart();
    if (!line.startsWith("- ")) list = null;
    if (line.startsWith("```")) {
      flush();
      const code = [];
      while (++index < lines.length && !lines[index].trimStart().startsWith("```")) code.push(lines[index]);
      target.append(element("pre", "at-code", code.join("\n")));
    } else if (line.startsWith("- ")) {
      flush();
      if (!list) target.append((list = element("ul")));
      list.append(element("li", null, line.slice(2)));
    } else if (line.startsWith("# ")) {
      flush();
      target.append(element("strong", null, line.slice(2)));
    } else if (!line) {
      flush();
    } else {
      paragraph.push(line);
    }
  }
  flush();
}

function copyButton(text, strings) {
  const button = element("button", "at-button at-button--ghost at-button--sm at-button--icon");
  button.dataset.tooltip = strings.copy;
  button.append(icon("copy"));
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(text());
      button.replaceChildren(icon("check"));
      setTimeout(() => button.replaceChildren(icon("copy")), 1500);
    } catch {
      // A page without clipboard access keeps its button as it was.
    }
  });
  return button;
}

/// Turn `root` (an empty element) into the conversation view `ChatView` is in gpui: it keeps the
/// entries, follows the end while the person is there, counts what arrives while they read
/// further up on a "jump to latest" button, groups consecutive tool calls, and fires
/// `atelier:decided` with `{ id, decision }` ("allow", "alwaysAllow" or "deny") when an approval
/// is answered.
///
/// Entries are plain objects with a `kind`: `user` (`text`, `attachments`), `assistant` (`text`,
/// `meta`, `streaming`), `reasoning` (`summary`, `text`), `tool` (`name`, `summary`, `state`:
/// "running" | "succeeded" | "failed" | "awaiting", `detail`, `icon`), `approval` (`title`,
/// `body`, `allowLabel`, `destructive`, `offerAlways`), `status`, `error` (`text`) or `custom`
/// (`element`). `{ panel: true }` fits it to a side panel; without, it is a centred column.
export function chat(root, { panel = false, logo = null, strings = {} } = {}) {
  const words = { ...CHAT_STRINGS, ...strings };
  root.classList.add("at-chat");
  root.classList.toggle("at-chat--panel", panel);
  const scroll = element("div", "at-chat__scroll");
  const transcript = element("div", "at-transcript");
  scroll.append(transcript);
  const jump = element("button", "at-button at-button--sm at-chat__jump");
  jump.hidden = true;
  root.replaceChildren(scroll, jump);

  const entries = new Map();
  let nextId = 0;
  let unread = 0;
  let following = true;

  const remaining = () => scroll.scrollHeight - scroll.clientHeight - scroll.scrollTop;
  const atTail = () => remaining() <= (following ? TAIL_SLACK : 1);
  const showJump = () => {
    jump.hidden = atTail() || entries.size === 0;
    jump.replaceChildren(icon("chevron-down"), unread ? words.newMessages(unread) : words.jumpToLatest);
  };
  // Run a change, then follow the end if the person was there, else count a new entry.
  const change = (apply, counts) => {
    const follow = atTail();
    const result = apply();
    following = follow;
    if (follow) {
      unread = 0;
      scroll.scrollTop = scroll.scrollHeight;
    } else if (counts) {
      unread += 1;
    }
    showJump();
    return result;
  };
  const jumpToLatest = () => {
    following = true;
    unread = 0;
    scroll.scrollTop = scroll.scrollHeight;
    showJump();
  };
  jump.addEventListener("click", jumpToLatest);
  // A view that was hidden, resized or grew from below keeps showing its end while followed.
  const keep = new ResizeObserver(() => {
    if (following) scroll.scrollTop = scroll.scrollHeight;
    showJump();
  });
  keep.observe(scroll);
  keep.observe(transcript);
  scroll.addEventListener("scroll", () => {
    following = atTail();
    if (following) unread = 0;
    showJump();
  });

  const avatar = () => {
    const mark = element("span", "at-avatar at-avatar--agent");
    if (logo) mark.append(icon(logo));
    return mark;
  };

  const draw = (entry) => {
    switch (entry.kind) {
      case "user": {
        const node = element("div", "at-message at-message--user");
        for (const label of entry.attachments || []) {
          const chip = element("span", "at-context-chip");
          chip.append(icon("file"), label);
          node.append(chip);
        }
        const actions = element("div", "at-message__actions");
        actions.append(copyButton(() => entry.text, words));
        node.append(element("div", "at-user-message", entry.text), actions);
        return { node };
      }
      case "assistant": {
        const node = element("div", "at-assistant-message");
        const header = element("div", "at-assistant-message__header");
        header.append(avatar(), element("strong", null, words.assistant));
        const meta = element("span", "at-faint", entry.meta || "");
        header.append(meta);
        const body = element("div", "at-assistant-message__body");
        const actions = element("div", "at-message__actions");
        actions.append(copyButton(() => entry.text, words));
        node.append(header, body, actions);
        const refresh = () => {
          prose(body, entry.text || "");
          if (entry.streaming) {
            (body.lastElementChild || body).append(element("span", "at-assistant-message__caret"));
          }
          actions.hidden = Boolean(entry.streaming);
          meta.textContent = entry.meta || "";
        };
        refresh();
        return { node, refresh };
      }
      case "reasoning": {
        const node = element("div", "at-stack");
        const summary = element("span", "at-thought");
        summary.tabIndex = 0;
        summary.append(icon("brain"), entry.summary, icon("chevron-right"));
        const text = element("div", "at-thought__text");
        text.hidden = true;
        const toggle = () => (text.hidden = !text.hidden);
        summary.addEventListener("click", toggle);
        summary.addEventListener("keydown", (event) => {
          if (event.key === "Enter" || event.key === " ") toggle(), event.preventDefault();
        });
        node.append(summary, text);
        const refresh = () => (text.textContent = entry.text || "");
        refresh();
        return { node, refresh };
      }
      case "tool": {
        const node = document.createDocumentFragment();
        const row = element("div", "at-tool-call");
        const detail = element("div", "at-tool-call__detail");
        detail.hidden = true;
        row.addEventListener("click", () => {
          if (entry.detail) row.setAttribute("aria-expanded", String((detail.hidden = !detail.hidden) === false));
        });
        const refresh = () => {
          const mark = {
            running: () => element("span", "at-spinner"),
            succeeded: () => Object.assign(icon("check"), { className: "at-icon at-tool-call__done" }),
            failed: () => Object.assign(icon("close"), { className: "at-icon at-tool-call__failed" }),
            awaiting: () => Object.assign(icon("alert"), { className: "at-icon at-tool-call__waiting" }),
          }[entry.state || "running"]();
          row.replaceChildren(
            icon(entry.icon || "wrench"),
            element("span", "at-tool-call__summary", entry.summary),
            element("span", "at-tool-call__name", entry.name),
            mark,
          );
          if (entry.detail) row.setAttribute("aria-expanded", String(!detail.hidden));
          detail.textContent = entry.detail || "";
        };
        refresh();
        node.append(row, detail);
        return { node, parts: [row, detail], refresh };
      }
      case "approval": {
        const node = element("div", "at-approval");
        const refresh = () => {
          if (entry.answer) {
            node.className = `at-approval at-approval--answered${entry.answer === "deny" ? " at-approval--denied" : ""}`;
            node.replaceChildren(
              icon(entry.answer === "deny" ? "close" : "check"),
              element("strong", null, words.answers[entry.answer]),
              element("span", null, entry.title),
            );
            return;
          }
          const title = element("strong");
          const alert = icon("alert");
          alert.style.color = "var(--at-warning)";
          title.append(alert, " ", entry.title);
          const actions = element("div", "at-approval__actions");
          const button = (label, decision, style) => {
            const b = element("button", `at-button at-button--sm ${style}`, label);
            b.addEventListener("click", () => answer(entry.id, decision, true));
            return b;
          };
          if (entry.offerAlways) actions.append(button(words.alwaysAllow, "alwaysAllow", "at-button--ghost"));
          actions.append(element("span", "at-spacer"), button(words.deny, "deny", ""));
          actions.append(button(entry.allowLabel || words.allow, "allow", entry.destructive ? "at-button--danger" : "at-button--primary"));
          node.replaceChildren(title);
          if (entry.body) node.append(element("span", "at-small at-muted", entry.body));
          node.append(actions);
        };
        refresh();
        return { node, refresh };
      }
      case "status":
      case "error": {
        const node = element("div", `at-notice${entry.kind === "error" ? " at-notice--error" : ""}`);
        if (entry.kind === "error") node.append(icon("alert"));
        node.append(entry.text);
        return { node };
      }
      default:
        return { node: entry.element || element("div") };
    }
  };

  // Consecutive tool calls share one group; its title counts them.
  const toolGroup = () => {
    const last = transcript.lastElementChild;
    if (last && last.classList.contains("at-tool-group")) return last;
    const group = element("div", "at-tool-group");
    group.append(element("div", "at-tool-group__title"));
    transcript.append(group);
    return group;
  };
  const retitle = (group) => {
    if (!group) return;
    const count = group.querySelectorAll(":scope > .at-tool-call").length;
    group.firstElementChild.textContent = words.usedTools(count);
    // A lone call stands on its own, unframed, as it does in gpui.
    group.classList.toggle("at-tool-group--single", count < 2);
  };

  const push = (input) => change(() => {
    const id = nextId++;
    const entry = { ...input, id };
    const drawn = draw(entry);
    const parts = drawn.parts || [drawn.node];
    if (entry.kind === "tool") {
      const group = toolGroup();
      group.append(drawn.node);
      retitle(group);
      entry.group = group;
    } else {
      transcript.append(drawn.node);
    }
    hydrate(transcript);
    entries.set(id, { entry, parts, refresh: drawn.refresh });
    return id;
  }, true);

  const update = (id, changeEntry) => {
    const held = entries.get(id);
    if (!held) return false;
    change(() => {
      changeEntry(held.entry);
      held.refresh?.();
    }, false);
    return true;
  };

  function answer(id, decision, fromClick = false) {
    update(id, (entry) => (entry.answer = decision));
    if (fromClick) root.dispatchEvent(new CustomEvent("atelier:decided", { bubbles: true, detail: { id, decision } }));
  }

  return {
    push,
    update,
    append: (id, delta) =>
      update(id, (entry) => {
        if (entry.kind === "tool") entry.detail = (entry.detail || "") + delta;
        else entry.text = (entry.text || "") + delta;
      }),
    finish: (id) => update(id, (entry) => (entry.streaming = false)),
    setToolState: (id, state) => update(id, (entry) => (entry.state = state)),
    answer: (id, decision) => answer(id, decision),
    remove(id) {
      const held = entries.get(id);
      if (!held) return;
      for (const part of held.parts) part.remove();
      if (held.entry.group) {
        if (!held.entry.group.querySelector(".at-tool-call")) held.entry.group.remove();
        else retitle(held.entry.group);
      }
      entries.delete(id);
      showJump();
    },
    clear() {
      entries.clear();
      transcript.replaceChildren();
      jumpToLatest();
    },
    jumpToLatest,
    get awaitingApproval() {
      return [...entries.values()].some(({ entry }) => entry.kind === "approval" && !entry.answer);
    },
    get unread() {
      return unread;
    },
    entries: () => [...entries.values()].map(({ entry }) => entry),
  };
}

// ---------------------------------------------------------------- dialogs and approvals

/// Open a modal dialog over the page, as `Dialog` is in gpui, and resolve with the `value` of the
/// action pressed, or `null` on Escape. `actions` run from the safe choice to the one the dialog
/// exists for: `[{ label, value, variant: "primary" | "danger" | "ghost", leading, focus }]`;
/// the one marked `focus` (else the first) takes focus. `content` is any extra element.
export function openDialog({ title, body, icon: iconName, tone = "warning", content, actions = [] }) {
  const dialog = element("dialog", "at-dialog");
  const heading = element("h2", "at-dialog__title");
  if (iconName) {
    const mark = icon(iconName);
    mark.style.color = `var(--at-${tone})`;
    heading.append(mark);
  }
  heading.append(title);
  dialog.append(heading);
  if (body) dialog.append(element("p", "at-dialog__body", body));
  if (content) dialog.append(content);
  const footer = element("div", "at-dialog__footer");
  const leading = actions.filter((action) => action.leading);
  const trailing = actions.filter((action) => !action.leading);
  const buttons = [];
  const button = (action) => {
    const variant = action.variant ? ` at-button--${action.variant}` : "";
    const b = element("button", `at-button${variant}`, action.label);
    b.disabled = Boolean(action.disabled);
    b.addEventListener("click", () => dialog.close(String(buttons.indexOf(b))));
    buttons.push(b);
    return { b, action };
  };
  const made = [...leading.map(button), ...trailing.map(button)];
  footer.append(...made.slice(0, leading.length).map(({ b }) => b), element("span", "at-spacer"));
  footer.append(...made.slice(leading.length).map(({ b }) => b));
  if (actions.length) dialog.append(footer);
  document.body.append(dialog);
  dialog.showModal();
  const first = made.find(({ action }) => action.focus) || made.find(({ b }) => !b.disabled);
  first?.b.focus();
  // A press on the scrim, outside the card, dismisses like Escape.
  dialog.addEventListener("mousedown", (event) => {
    if (event.target === dialog) {
      const box = dialog.getBoundingClientRect();
      const inside = event.clientX >= box.left && event.clientX <= box.right && event.clientY >= box.top && event.clientY <= box.bottom;
      if (!inside) dialog.close("");
    }
  });
  return new Promise((resolve) => {
    dialog.addEventListener("close", () => {
      const index = dialog.returnValue === "" ? -1 : Number(dialog.returnValue);
      dialog.remove();
      resolve(index >= 0 ? made[index].action.value : null);
    });
  });
}

/// An approval as a modal dialog, as `ApprovalDialog` is in gpui: resolves with "allow",
/// "alwaysAllow" or "deny". Deny takes focus, and Escape or a press outside denies. With
/// `reviewing: true` only Deny can be pressed while an automatic reviewer decides.
export async function approvalDialog({ title, body, command, detail, allowLabel, destructive = false, offerAlways = false, reviewing = false, strings = {} }) {
  const words = { ...CHAT_STRINGS, reviewing: "Reviewing…", ...strings };
  const content = document.createDocumentFragment();
  if (command) content.append(element("pre", "at-code", `$ ${command}`));
  if (detail) content.append(detail);
  if (reviewing) {
    const note = element("div", "at-dialog__reviewing");
    note.append(element("span", "at-spinner"), words.reviewing);
    content.append(note);
  }
  const actions = [];
  if (offerAlways) actions.push({ label: words.alwaysAllow, value: "alwaysAllow", variant: "ghost", leading: true, disabled: reviewing });
  actions.push({ label: words.deny, value: "deny", focus: true });
  actions.push({ label: allowLabel || words.allow, value: "allow", variant: destructive ? "danger" : "primary", disabled: reviewing });
  const decision = await openDialog({ title, body, icon: "alert", tone: destructive ? "danger" : "warning", content, actions });
  return decision || "deny";
}

/// The permission modes, from the most careful to the least, as `PermissionMode` in gpui.
export const PERMISSION_MODES = {
  plan: { label: "Plan only", detail: "Reads and proposes. Changes nothing.", icon: "eye" },
  ask: { label: "Ask before changes", detail: "Asks before every edit and command.", icon: "user" },
  acceptEdits: { label: "Accept edits", detail: "Edits files without asking. Asks before commands.", icon: "file" },
  autoReview: { label: "Auto-review", detail: "A reviewer model answers each request. Unsure ones come to you.", icon: "sparkles" },
  bypass: { label: "Bypass approvals", detail: "Runs everything without asking. Use only in a sandbox.", icon: "alert", warns: true },
};

/// Turn `root` (an empty element) into the permission picker `PermissionPicker` is in gpui: a
/// small button showing the mode, opening a menu of `modes` upwards. Fires `atelier:permission`
/// with `{ mode }` on a pick. A mode that `warns` keeps the button in the danger colour. Pass
/// `labels` to translate: `{ heading, plan: { label, detail }, … }`.
export function permissionPicker(root, { mode = "ask", modes = Object.keys(PERMISSION_MODES), labels = {} } = {}) {
  const info = (id) => ({ ...PERMISSION_MODES[id], ...labels[id] });
  root.classList.add("at-permission");
  const button = element("button", "at-permission__button");
  button.type = "button";
  button.setAttribute("aria-haspopup", "menu");
  const menu = element("div", "at-menu");
  menu.setAttribute("role", "menu");
  menu.hidden = true;
  root.replaceChildren(menu, button);
  const draw = () => {
    const current = info(mode);
    root.classList.toggle("at-permission--warns", Boolean(current.warns));
    button.replaceChildren(icon(current.icon), current.label, icon("chevron-down"));
    button.setAttribute("aria-expanded", String(!menu.hidden));
    menu.replaceChildren(element("div", "at-caption", labels.heading || "Permissions"));
    for (const id of modes) {
      const option = info(id);
      const item = element("button", `at-menu-item at-menu-item--described${option.warns ? " at-menu-item--danger" : ""}`);
      item.setAttribute("role", "menuitemradio");
      item.setAttribute("aria-checked", String(id === mode));
      const check = element("span", "at-menu-item__check");
      if (id === mode) check.append(icon("check"));
      const text = element("span", "at-menu-item__text");
      text.append(element("span", null, option.label), element("span", "at-menu-item__description", option.detail));
      item.append(check, icon(option.icon), text);
      item.addEventListener("click", () => {
        mode = id;
        close();
        root.dispatchEvent(new CustomEvent("atelier:permission", { bubbles: true, detail: { mode } }));
      });
      menu.append(item);
    }
  };
  const close = () => {
    menu.hidden = true;
    draw();
  };
  button.addEventListener("click", () => {
    menu.hidden = !menu.hidden;
    draw();
    if (!menu.hidden) menu.querySelector('[aria-checked="true"]')?.focus();
  });
  root.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !menu.hidden) {
      close();
      button.focus();
      event.stopPropagation();
    }
  });
  document.addEventListener("mousedown", (event) => {
    if (!menu.hidden && !root.contains(event.target)) close();
  });
  draw();
  return {
    get mode() {
      return mode;
    },
    set mode(value) {
      mode = value;
      draw();
    },
  };
}

/// A running turn's phases, as `agent::AgentPhase` names them.
export const AGENT_PHASES = {
  waiting: "Waiting",
  reading: "Reading",
  thinking: "Thinking",
  writing: "Writing",
  tool: "Running",
  approval: "Needs approval",
  compacting: "Compacting",
  reviewing: "Reviewing",
};

/// "12 s", "1:05", "1:02:05", as `AgentActivity` writes elapsed time.
function formatElapsed(seconds) {
  seconds = Math.floor(seconds);
  const pad = (n) => String(n).padStart(2, "0");
  if (seconds < 60) return `${seconds} s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}:${pad(seconds % 60)}`;
  return `${Math.floor(seconds / 3600)}:${pad(Math.floor(seconds / 60) % 60)}:${pad(seconds % 60)}`;
}

/// "820", "1.2k", "34k", "1.5M".
function formatCount(count) {
  if (count < 1000) return String(count);
  if (count < 10000) return `${(count / 1000).toFixed(1)}k`;
  if (count < 1000000) return `${Math.floor(count / 1000)}k`;
  return `${(count / 1000000).toFixed(1)}M`;
}

/// `agent::AgentActivity`: draw what a running turn is doing into `root`. Call again with new
/// figures as they come; every one but `phase` is optional. `elapsed` is in seconds,
/// `context` and `progress` are 0–1.
export function activity(root, { phase = "waiting", detail, progress, elapsed, rate, tokens, context, labels = {} } = {}) {
  const approval = phase === "approval";
  root.className = `at-activity${approval ? " at-activity--approval" : ""}`;
  root.replaceChildren();
  const lead = element("span", "at-activity__phase");
  lead.append(approval ? element("span", "at-status-dot at-status-dot--warning") : element("span", "at-spinner"));
  let label = labels[phase] || AGENT_PHASES[phase] || phase;
  if (progress != null) label += ` ${Math.round(progress * 100)} %`;
  lead.append(label);
  root.append(lead, element("span", "at-activity__detail", detail || ""));
  if (tokens != null) root.append(element("span", "at-activity__figure", `${formatCount(tokens)} tok`));
  if (rate != null) root.append(element("span", "at-activity__figure", `${Math.round(rate)} tok/s`));
  if (elapsed != null) root.append(element("span", "at-activity__figure", formatElapsed(elapsed)));
  if (context != null) {
    const tone = context >= 0.95 ? " at-activity__context--danger" : context >= 0.8 ? " at-activity__context--warning" : "";
    const gauge = element("span", `at-activity__context${tone}`);
    gauge.style.setProperty("--value", String(context));
    gauge.append(element("span", "at-activity__figure", `${Math.round(context * 100)} %`));
    root.append(gauge);
  }
  return root;
}

/// `agent::PlanView`: draw the agent's plan into `root`, replacing what was there. Each step is
/// `{ label, detail, state }` with `state` one of pending, running, done, failed, skipped. The
/// header folds it to the step in progress and fires `atelier:toggle` with `{ open }`.
export function planView(root, { title = "Plan", steps = [], open } = {}) {
  const closed = open === undefined ? root.classList.contains("at-plan--closed") : !open;
  root.className = `at-plan${closed ? " at-plan--closed" : ""}`;
  root.replaceChildren();
  const settled = steps.filter((step) => step.state === "done" || step.state === "skipped").length;
  const header = element("button", "at-plan__header");
  header.type = "button";
  const bar = element("span", `at-progress${settled === steps.length && steps.length ? " at-progress--done" : ""}`);
  const fill = element("span");
  fill.style.width = `${steps.length ? (settled / steps.length) * 100 : 0}%`;
  bar.append(fill);
  header.append(element("span", "", title), element("span", "at-plan__count", `${settled}/${steps.length}`), bar);
  const list = element("ol", "at-steps");
  for (const step of steps) {
    const item = element("li", `at-step at-step--${step.state || "pending"}`);
    item.append(element("span", "", step.label));
    if (step.detail) item.append(element("span", "at-step__detail", step.detail));
    list.append(item);
  }
  root.append(header, list);
  foldablePlan(root);
  return root;
}

/// `agent::ChangedFiles`: draw the files a turn changed into `root`. Each file is
/// `{ path, kind, added, removed }` with `kind` one of A, M (the default), D, R. Pressing a row
/// fires `atelier:open-file` with `{ index, path }`. `actions` go at the header's end.
export function changedFiles(root, { title = "Changed files", files = [], actions = [] } = {}) {
  root.className = "at-changes";
  root.replaceChildren();
  const lines = (added, removed) => {
    const counts = element("span", "at-changes__lines");
    counts.append(element("span", "", `+${added}`), element("span", "", `−${removed}`));
    return counts;
  };
  const total = files.reduce((sum, file) => [sum[0] + (file.added || 0), sum[1] + (file.removed || 0)], [0, 0]);
  const header = element("div", "at-changes__header");
  header.append(
    element("span", "at-changes__title", title),
    element("span", "at-changes__count", String(files.length)),
    lines(...total),
    ...actions,
  );
  root.append(header);
  files.forEach((file, index) => {
    const row = element("button", "at-change");
    row.type = "button";
    row.dataset.kind = file.kind || "M";
    const path = file.path.replace(/\/+$/, "");
    const at = path.lastIndexOf("/");
    const name = element("span", "at-change__path");
    name.append(element("span", "at-change__name", path.slice(at + 1)), element("span", "at-change__folder", at < 0 ? "" : path.slice(0, at)));
    row.append(name, lines(file.added || 0, file.removed || 0));
    row.title = path;
    row.addEventListener("click", () =>
      root.dispatchEvent(new CustomEvent("atelier:open-file", { bubbles: true, detail: { index, path } })),
    );
    root.append(row);
  });
  return root;
}

/// `graph::NodeCanvas`: a node graph on a pannable canvas in `root` (give it a height).
///
/// Each node is `{ title, icon, x, y, width, inputs, outputs, fields, state, progress, seconds,
/// byAgent }`: ports are `{ name, kind }` with `kind` an index into `portTypes` (it picks the
/// port colour), fields are `{ type: "number" | "choice" | "text", name, value, fraction }`,
/// and `state` is idle, queued, running (with `progress` 0–1), done (with `seconds`), cached or
/// failed. Wires are `{ from, output, to, input }` by node id, and only join ports of one kind.
///
/// The person drags nodes, draws wires from an output to an input, picks a wire up by its
/// input, pans with the background or the wheel, and deletes the selected node with Delete.
/// What they do fires `atelier:node-select`, `node-move`, `node-connect`, `node-disconnect`,
/// `node-remove`, `node-keep` and `node-undo` on `root`, after the canvas has applied it.
export function nodeCanvas(root, { portTypes = [], nodes = [], wires = [], legend = true, labels = {} } = {}) {
  const words = { addedByAgent: "Added by the assistant", keep: "Keep", undo: "Undo", resetView: "Reset view", queued: "Queued", cached: "Cached", ...labels };
  const graph = new Map();
  let links = [];
  let next = 0;
  let pan = { x: 16, y: 24 };
  let selected = null;
  let drag = null;
  root.classList.add("at-node-canvas");
  root.tabIndex = 0;
  root.replaceChildren();
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.classList.add("at-node-canvas__wires");
  const layer = element("div", "at-node-canvas__layer");
  root.append(svg, layer);
  const emit = (name, detail) => root.dispatchEvent(new CustomEvent(`atelier:${name}`, { bubbles: true, detail }));
  const color = (kind) => `var(--at-port-${(kind || 0) % 5})`;

  if (legend) {
    const box = element("div", "at-node-canvas__legend");
    const reset = element("button", "at-button at-button--ghost at-button--sm", words.resetView);
    reset.type = "button";
    reset.addEventListener("click", () => api.resetView());
    box.append(reset);
    portTypes.forEach((name, kind) => {
      const item = element("span");
      const dot = element("span", "at-status-dot");
      dot.style.background = color(kind);
      item.append(dot, name);
      box.append(item);
    });
    root.append(box);
  }

  const portKind = (id, index, output) => graph.get(id)?.[output ? "outputs" : "inputs"]?.[index]?.kind;
  const canConnect = (wire) =>
    wire.from !== wire.to && portKind(wire.from, wire.output, true) !== undefined && portKind(wire.from, wire.output, true) === portKind(wire.to, wire.input, false);
  const same = (a, b) => a.from === b.from && a.output === b.output && a.to === b.to && a.input === b.input;

  function stateMark(node) {
    const mark = element("span", "at-node__state");
    switch (node.state) {
      case "queued":
        mark.textContent = words.queued;
        break;
      case "running":
        mark.append(`${Math.round((node.progress || 0) * 100)} %`, element("span", "at-spinner"));
        break;
      case "done":
        mark.classList.add("at-node__state--done");
        if (node.seconds != null) mark.textContent = `${node.seconds.toFixed(1)} s`;
        break;
      case "cached":
        mark.textContent = words.cached;
        break;
      case "failed":
        mark.classList.add("at-node__state--failed");
        mark.textContent = "!";
        break;
      default:
        if (node.byAgent) {
          mark.classList.add("at-node__state--agent");
          mark.append(icon("sparkles", { size: 12 }));
        }
    }
    return mark;
  }

  function drawNode(id, node) {
    const card = element("div", "at-node");
    card.dataset.node = id;
    card.style.left = `${node.x || 0}px`;
    card.style.top = `${node.y || 0}px`;
    card.style.width = `${node.width || 200}px`;
    card.classList.toggle("at-node--agent", !!node.byAgent);
    card.classList.toggle("is-selected", selected === id);
    card.classList.toggle("is-running", node.state === "running");
    card.classList.toggle("is-failed", node.state === "failed");
    const header = element("div", "at-node__header");
    if (node.icon) header.append(icon(node.icon));
    header.append(element("span", "at-node__title", node.title), stateMark(node));
    card.append(header);
    if (node.state === "running") {
      const bar = element("div", "at-node__progress");
      const fill = element("span");
      fill.style.width = `${(node.progress || 0) * 100}%`;
      bar.append(fill);
      card.append(bar);
    }
    const inputs = node.inputs || [];
    const outputs = node.outputs || [];
    const rows = Math.max(inputs.length, outputs.length);
    if (rows) {
      const ports = element("div", "at-node__ports");
      for (let row = 0; row < rows; row++) {
        const line = element("div", "at-node__row");
        line.append(element("span", "", inputs[row]?.name || ""), element("span", "", outputs[row]?.name || ""));
        for (const [list, output] of [[inputs, false], [outputs, true]]) {
          const port = list[row];
          if (!port) continue;
          const socket = element("span", `at-node__socket at-node__socket--${output ? "out" : "in"}`);
          socket.dataset.kind = port.kind || 0;
          socket.dataset.index = row;
          socket.title = portTypes[port.kind] ? `${port.name} · ${portTypes[port.kind]}` : port.name;
          const wired = links.some((wire) => (output ? wire.from === id && wire.output === row : wire.to === id && wire.input === row));
          socket.classList.toggle("is-wired", wired);
          line.append(socket);
        }
        ports.append(line);
      }
      card.append(ports);
    }
    if (node.fields?.length) {
      const fields = element("div", "at-node__fields");
      for (const field of node.fields) {
        const row = element("div", `at-node__field at-node__field--${field.type || "number"}`);
        if (field.fraction != null) row.style.setProperty("--fraction", String(field.fraction));
        row.append(element("span", "", field.name), element("span", "", field.value ?? ""));
        fields.append(row);
      }
      card.append(fields);
    }
    if (node.byAgent) {
      const footer = element("div", "at-node__agent", words.addedByAgent);
      const buttons = element("div");
      const undo = element("button", "at-button at-button--ghost at-button--sm", words.undo);
      const keep = element("button", "at-button at-button--sm", words.keep);
      undo.type = keep.type = "button";
      undo.addEventListener("click", () => {
        api.undoAgentNode(id);
        emit("node-undo", { id });
      });
      keep.addEventListener("click", () => {
        api.update(id, { byAgent: false });
        emit("node-keep", { id });
      });
      buttons.append(undo, keep);
      footer.append(buttons);
      card.append(footer);
    }
    return card;
  }

  function socketCentre(id, index, output) {
    const socket = layer.querySelector(`.at-node[data-node="${id}"] .at-node__socket--${output ? "out" : "in"}[data-index="${index}"]`);
    if (!socket) return null;
    const box = socket.getBoundingClientRect();
    const frame = root.getBoundingClientRect();
    return { x: box.left + box.width / 2 - frame.left, y: box.top + box.height / 2 - frame.top };
  }

  function curve(from, to) {
    const reach = Math.max(Math.abs(to.x - from.x) * 0.5, 48);
    return `M ${from.x} ${from.y} C ${from.x + reach} ${from.y}, ${to.x - reach} ${to.y}, ${to.x} ${to.y}`;
  }

  function drawWires() {
    svg.replaceChildren();
    const add = (from, to, kind, strong) => {
      if (!from || !to) return;
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", curve(from, to));
      path.style.stroke = color(kind);
      if (strong) path.classList.add("is-strong");
      svg.append(path);
    };
    for (const wire of links) {
      add(socketCentre(wire.from, wire.output, true), socketCentre(wire.to, wire.input, false), portKind(wire.from, wire.output, true), selected === wire.from || selected === wire.to);
    }
    if (drag?.type === "wire") add(socketCentre(drag.from, drag.output, true), drag.cursor, portKind(drag.from, drag.output, true), true);
  }

  function draw() {
    root.style.setProperty("--_pan-x", `${pan.x}px`);
    root.style.setProperty("--_pan-y", `${pan.y}px`);
    layer.replaceChildren(...[...graph].map(([id, node]) => drawNode(id, node)));
    // While a wire is dragged, inputs it cannot join step back.
    if (drag?.type === "wire") {
      for (const socket of layer.querySelectorAll(".at-node__socket--in")) {
        const to = Number(socket.closest(".at-node").dataset.node);
        socket.classList.toggle("is-muted", !canConnect({ from: drag.from, output: drag.output, to, input: Number(socket.dataset.index) }));
      }
    }
    drawWires();
  }

  const point = (event) => {
    const frame = root.getBoundingClientRect();
    return { x: event.clientX - frame.left, y: event.clientY - frame.top };
  };

  root.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 && event.button !== 1) return;
    if (event.target.closest("button, .at-node-canvas__legend")) return;
    root.focus({ preventScroll: true });
    const socket = event.target.closest(".at-node__socket");
    const card = event.target.closest(".at-node");
    if (socket && card) {
      const id = Number(card.dataset.node);
      const index = Number(socket.dataset.index);
      if (socket.classList.contains("at-node__socket--out")) {
        drag = { type: "wire", from: id, output: index, cursor: point(event) };
      } else {
        // Pulling a wired input picks its wire up again.
        const wire = links.find((wire) => wire.to === id && wire.input === index);
        if (!wire) return;
        links = links.filter((other) => other !== wire);
        emit("node-disconnect", { wire });
        drag = { type: "wire", from: wire.from, output: wire.output, cursor: point(event) };
      }
    } else if (card && event.button === 0) {
      const id = Number(card.dataset.node);
      if (selected !== id) {
        api.select(id);
        emit("node-select", { id });
      }
      drag = { type: "node", id, last: point(event), moved: false };
    } else {
      if (event.button === 0 && selected !== null) {
        api.select(null);
        emit("node-select", { id: null });
      }
      drag = { type: "pan", last: point(event) };
    }
    root.setPointerCapture(event.pointerId);
    draw();
    event.preventDefault();
  });

  root.addEventListener("pointermove", (event) => {
    if (!drag) return;
    const at = point(event);
    if (drag.type === "pan") {
      pan = { x: pan.x + at.x - drag.last.x, y: pan.y + at.y - drag.last.y };
      drag.last = at;
      draw();
    } else if (drag.type === "node") {
      const node = graph.get(drag.id);
      node.x = (node.x || 0) + at.x - drag.last.x;
      node.y = (node.y || 0) + at.y - drag.last.y;
      drag.last = at;
      drag.moved = true;
      const card = layer.querySelector(`.at-node[data-node="${drag.id}"]`);
      card.style.left = `${node.x}px`;
      card.style.top = `${node.y}px`;
      card.classList.add("is-dragging");
      drawWires();
    } else {
      drag.cursor = at;
      drawWires();
    }
  });

  const finish = (event) => {
    if (!drag) return;
    const ended = drag;
    drag = null;
    if (ended.type === "node" && ended.moved) emit("node-move", { id: ended.id });
    if (ended.type === "wire") {
      const target = document.elementFromPoint(event.clientX, event.clientY)?.closest(".at-node__socket--in");
      const card = target?.closest(".at-node");
      if (target && card) {
        const wire = { from: ended.from, output: ended.output, to: Number(card.dataset.node), input: Number(target.dataset.index) };
        if (canConnect(wire)) {
          const replaced = links.find((old) => old.to === wire.to && old.input === wire.input);
          links = links.filter((old) => old !== replaced);
          links.push(wire);
          if (replaced) emit("node-disconnect", { wire: replaced });
          emit("node-connect", { wire });
        }
      }
    }
    draw();
  };
  root.addEventListener("pointerup", finish);
  root.addEventListener("pointercancel", finish);

  root.addEventListener(
    "wheel",
    (event) => {
      pan = { x: pan.x - event.deltaX, y: pan.y - event.deltaY };
      draw();
      event.preventDefault();
    },
    { passive: false },
  );

  root.addEventListener("keydown", (event) => {
    if ((event.key === "Delete" || event.key === "Backspace") && selected !== null) {
      const id = selected;
      api.remove(id);
      emit("node-remove", { id });
      event.preventDefault();
    } else if (event.key === "Escape" && selected !== null) {
      api.select(null);
      emit("node-select", { id: null });
    }
  });

  const api = {
    add(node) {
      const id = next++;
      graph.set(id, { ...node });
      draw();
      return id;
    },
    remove(id) {
      graph.delete(id);
      links = links.filter((wire) => wire.from !== id && wire.to !== id);
      if (selected === id) selected = null;
      draw();
    },
    update(id, patch) {
      const node = graph.get(id);
      if (!node) return;
      Object.assign(node, patch);
      draw();
    },
    setState(id, state, { progress, seconds } = {}) {
      api.update(id, { state, progress, seconds });
    },
    connect(wire) {
      if (!canConnect(wire)) return false;
      links = links.filter((old) => !(old.to === wire.to && old.input === wire.input));
      links.push({ ...wire });
      draw();
      return true;
    },
    disconnect(wire) {
      links = links.filter((old) => !same(old, wire));
      draw();
    },
    select(id) {
      selected = id;
      // The touched node comes to the front.
      if (id !== null && graph.has(id)) {
        const node = graph.get(id);
        graph.delete(id);
        graph.set(id, node);
      }
      draw();
    },
    undoAgentNode(id) {
      const source = links.find((wire) => wire.to === id);
      const targets = links.filter((wire) => wire.from === id);
      api.remove(id);
      if (source) for (const target of targets) api.connect({ from: source.from, output: source.output, to: target.to, input: target.input });
    },
    resetView() {
      pan = { x: 16, y: 24 };
      draw();
    },
    get selected() {
      return selected;
    },
    nodes: () => [...graph].map(([id, node]) => ({ id, ...node })),
    wires: () => links.map((wire) => ({ ...wire })),
    canConnect,
    downstream(id) {
      const found = [id];
      for (let index = 0; index < found.length; index++) {
        for (const wire of links) if (wire.from === found[index] && !found.includes(wire.to)) found.push(wire.to);
      }
      return found;
    },
    executionOrder() {
      const remaining = [...graph.keys()].sort((a, b) => a - b);
      const order = [];
      while (remaining.length) {
        const ready = remaining.findIndex((id) => links.filter((wire) => wire.to === id).every((wire) => !remaining.includes(wire.from)));
        order.push(remaining.splice(Math.max(ready, 0), 1)[0]);
      }
      return order;
    },
  };

  for (const node of nodes) api.add(node);
  for (const wire of wires) api.connect(wire);
  // Wires follow the cards' measured sockets, so draw again once fonts and layout settle.
  requestAnimationFrame(draw);
  if (typeof ResizeObserver !== "undefined") new ResizeObserver(() => drawWires()).observe(root);
  return api;
}
