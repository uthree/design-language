// The ambient backdrops on the web: the native scenes' own WGSL shaders, run on WebGPU.
//
// Each frame fills the shaders' `Params` the way `ambient/gpu.rs` does: the frame, the cycle, the
// theme's palette, and the pointer, eased and fading in and out, with ripples left along its
// path as `ambient/sim.rs` leaves them. Where there is no WebGPU the canvas keeps the CSS
// gradient `.at-ambient` draws, so a page never waits on a GPU to look finished.

import { common, layout, palettes, scenes } from "./generated/ambient.js";

/// The scenes the web can draw, in the order a picker should offer them.
export const AMBIENCES = Object.keys(scenes);

// As in `sim.rs`.
const FOLLOW = 7;
const FADE = 2.5;
const SPEED_FOLLOW = 5;
const WAVE_SPACING = 0.09;
const WAVE_SPEED = 0.32;
const WAVE_LIFE = 3;
// The frame rate while the scene only drifts; the pointer gets every frame.
const DRIFT_FPS = 30;

export class AmbientBackdrop {
  /// Draw `scene` into `canvas`, in the colours of the theme on `<html data-theme>` or of
  /// `theme` if given. Resolves to `null` where WebGPU is missing.
  static async create(canvas, { scene = "Aurora", theme } = {}) {
    if (!navigator.gpu) return null;
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: "low-power" });
    if (!adapter) return null;
    const device = await adapter.requestDevice();
    return new AmbientBackdrop(canvas, device, scene, theme);
  }

  constructor(canvas, device, scene, theme) {
    this.canvas = canvas;
    this.device = device;
    this.context = canvas.getContext("webgpu");
    this.format = navigator.gpu.getPreferredCanvasFormat();
    this.context.configure({ device, format: this.format, alphaMode: "opaque" });
    this.params = new ArrayBuffer(layout.length * 4);
    this.floats = new Float32Array(this.params);
    this.words = new Uint32Array(this.params);
    this.buffer = device.createBuffer({
      size: this.params.byteLength,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    this.layout = device.createBindGroupLayout({
      entries: [{ binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: "uniform" } }],
    });
    this.bindings = device.createBindGroup({
      layout: this.layout,
      entries: [{ binding: 0, resource: { buffer: this.buffer } }],
    });
    this.pipelines = new Map();
    this.themeOverride = theme;
    this.scene = scene;
    this.start = performance.now();
    this.last = null;
    this.lastDrawn = 0;
    this.focus = [0, 0];
    this.presence = 0;
    this.velocity = [0, 0];
    this.pointer = null;
    this.previous = null;
    this.travel = 0;
    this.waves = [];
    this.running = false;

    this.onMove = (event) => {
      const box = canvas.getBoundingClientRect();
      this.pointer = [event.clientX - box.left, event.clientY - box.top];
    };
    this.onLeave = () => {
      this.pointer = null;
    };
    window.addEventListener("pointermove", this.onMove, { passive: true });
    document.documentElement.addEventListener("pointerleave", this.onLeave);
    this.resize = new ResizeObserver(() => this.fit());
    this.resize.observe(canvas);
    this.fit();
  }

  /// Change the scene.
  setScene(scene) {
    if (scenes[scene]) this.scene = scene;
  }

  /// Follow the page's theme again, or `theme` if given.
  setTheme(theme) {
    this.themeOverride = theme;
  }

  /// Start drawing, or (when `still`) draw one frame at `phase` of the cycle.
  play() {
    if (this.running) return;
    this.running = true;
    const tick = (now) => {
      if (!this.running) return;
      this.frame(now);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  pause() {
    this.running = false;
  }

  /// One frame at `phase` (0 to 1) of the cycle, with the pointer away: for screenshots and for
  /// a reduced-motion preference.
  still(phase = 0.12) {
    this.pointer = null;
    this.presence = 0;
    this.waves = [];
    this.draw(phase * Math.PI * 2);
  }

  destroy() {
    this.pause();
    this.resize.disconnect();
    window.removeEventListener("pointermove", this.onMove);
    document.documentElement.removeEventListener("pointerleave", this.onLeave);
  }

  fit() {
    // One shader pixel per CSS pixel, as the native backdrops shade one per logical pixel.
    const box = this.canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(box.width));
    const height = Math.max(1, Math.round(box.height));
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
      if (!this.running) this.still();
    }
  }

  pipeline(scene) {
    let pipeline = this.pipelines.get(scene);
    if (!pipeline) {
      const module = this.device.createShaderModule({ code: common + scenes[scene].shader });
      pipeline = this.device.createRenderPipeline({
        layout: this.device.createPipelineLayout({ bindGroupLayouts: [this.layout] }),
        vertex: { module, entryPoint: "vs" },
        fragment: { module, entryPoint: "fs", targets: [{ format: this.format }] },
      });
      this.pipelines.set(scene, pipeline);
    }
    return pipeline;
  }

  frame(now) {
    const dt = this.last === null ? 0 : Math.min((now - this.last) / 1000, 0.1);
    this.last = now;
    this.step(dt);
    // Drift at a calm rate; answer the pointer at the display's.
    const lively = this.presence > 0 || this.waves.length > 0;
    if (!lively && now - this.lastDrawn < 1000 / DRIFT_FPS) return;
    this.lastDrawn = now;
    const period = scenes[this.scene].period;
    const seconds = (now - this.start) / 1000;
    this.draw(((seconds % period) / period) * Math.PI * 2);
  }

  step(dt) {
    const size = [this.canvas.width, this.canvas.height];
    const unit = Math.max(1, Math.min(size[0], size[1]));
    const { pointer, previous } = this;
    let speed = [0, 0];
    if (pointer && previous && dt > 0) {
      speed = [(pointer[0] - previous[0]) / dt, (pointer[1] - previous[1]) / dt];
      this.travel += Math.hypot(pointer[0] - previous[0], pointer[1] - previous[1]);
      if (this.travel > WAVE_SPACING * unit) {
        this.travel = 0;
        if (this.waves.length === layout.waveCount) this.waves.shift();
        this.waves.push({
          centre: pointer,
          age: 0,
          strength: Math.min(1, Math.max(0.25, Math.hypot(...speed) / (1.2 * unit))),
        });
      }
    }
    this.previous = pointer;
    const approach = (from, to, rate) => {
      const t = 1 - Math.exp(-rate * dt);
      return [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t];
    };
    if (pointer) {
      if (this.presence < 0.01) this.focus = pointer;
      this.focus = approach(this.focus, pointer, FOLLOW);
    }
    const present = pointer ? 1 : 0;
    this.presence += (present - this.presence) * (1 - Math.exp(-FADE * dt));
    if (Math.abs(present - this.presence) < 0.002) this.presence = present;
    this.velocity = approach(this.velocity, speed, SPEED_FOLLOW);
    if (Math.hypot(...this.velocity) < 1) this.velocity = [0, 0];
    for (const wave of this.waves) wave.age += dt;
    this.waves = this.waves.filter((wave) => wave.age < WAVE_LIFE);
  }

  draw(time) {
    const { floats, words } = this;
    const width = this.canvas.width;
    const height = this.canvas.height;
    const unit = Math.max(1, Math.min(width, height));
    const place = ([x, y]) => [(x - width / 2) / unit, (y - height / 2) / unit];
    const themeId = this.themeOverride || document.documentElement.dataset.theme || "glass-dark";
    const palette = palettes[themeId] || palettes["glass-dark"];
    const answers = this.scene !== "Dunes";

    floats.fill(0);
    const [px, py] = place(this.focus);
    floats.set([width, height, time, palette.dark ? 1 : 0, px, py, answers ? this.presence : 0], 0);
    words[7] = answers ? this.waves.length : 0;
    floats.set([this.velocity[0] / unit, this.velocity[1] / unit, 0, 0], 8);
    floats.set(palette.base, 12);
    floats.set(palette.deep, 16);
    palette.hues.forEach((hue, index) => floats.set(hue, 20 + index * 4));
    this.waves.forEach((wave, index) => {
      const life = wave.age / WAVE_LIFE;
      const [x, y] = place(wave.centre);
      floats.set(
        [x, y, WAVE_SPEED * wave.age, wave.strength * (1 - life) * (1 - life)],
        layout.waves + index * 4,
      );
    });
    // Aurora's brighter stars come from a simulation the web doesn't run yet: keep them off
    // the frame rather than heaped at its centre.
    for (let index = layout.stars; index < layout.lines; index += 4) {
      floats.set([1e3, 1e3, 0, 0], index);
    }
    this.device.queue.writeBuffer(this.buffer, 0, this.params);

    const encoder = this.device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: this.context.getCurrentTexture().createView(),
          loadOp: "clear",
          clearValue: { r: 0, g: 0, b: 0, a: 1 },
          storeOp: "store",
        },
      ],
    });
    pass.setPipeline(this.pipeline(this.scene));
    pass.setBindGroup(0, this.bindings);
    pass.draw(3);
    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }
}
