# Atelier

A design language and component kit for [gpui](https://crates.io/crates/gpui) creator tools with
built-in AI agents: theme tokens in three candidate directions (Glass, adopted; Studio and Mono
as alternatives), basic controls, creator controls (knobs, meters, transport), and the agent side panel, transcript and
composer. It ships as a Rust crate and as an agent Skill for Claude Code and Codex.

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

More examples — a coding-agent app, a physical-model instrument view, an analyser, and two
Auris Studio windows redrawn — are in [doc/examples.md](doc/examples.md). Install the Skill: see
[doc/skill.md](doc/skill.md). The candidates are compared in
[doc/candidates.md](doc/candidates.md); the rules and API live in
[skills/atelier-gpui](skills/atelier-gpui/SKILL.md).

## License

Apache-2.0
