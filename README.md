# Atelier

A design language and component kit for [gpui](https://crates.io/crates/gpui) creator tools with
built-in AI agents: theme tokens in three candidate directions (Glass, adopted; Studio and Mono
as alternatives), basic controls, creator controls (knobs, meters, transport), audio visualisations, 2-D and 3-D
plots, welcome-screen backdrops, and the agent side panel, transcript and composer. It ships as a Rust crate and as an agent Skill for Claude Code and Codex.

![Glass light](doc/images/arrange-glass-light.png)

## Install

```toml
[dependencies]
gpui = "=0.2.2"
atelier = { git = "https://github.com/uthree/design-language" }
```

Try the mock creator tool used to compare the candidates:

```bash
cargo run --example showcase -- --direction glass --appearance dark
```

More examples — a coding-agent app, a physical-model instrument view, an analyser, a node-based
workflow editor, a MATLAB-style plotting workbench with 3-D views, welcome screens over liquid
metal and ambient backdrops, two Auris Studio windows redrawn, a preferences window and a legend of which control
handles which concept — are in [doc/examples.md](doc/examples.md). Install the Skill: see
[doc/skill.md](doc/skill.md). The candidates are compared in
[doc/candidates.md](doc/candidates.md); app icons and logos, two-letter element marks such as
Auris Studio's **Au**, are set out in [doc/app-icons.md](doc/app-icons.md); the rules and API live in
[skills/atelier-gpui](skills/atelier-gpui/SKILL.md).

For a browser or a Tauri webview, [`web/`](web/README.md) carries the same themes, components and
ambient backdrops as CSS, a little JavaScript and WebGPU, with the tokens written from the Rust
theme.

## License

Apache-2.0
