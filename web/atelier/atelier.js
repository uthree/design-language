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
