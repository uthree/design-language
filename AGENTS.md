# Working on Atelier

Rules for anyone changing this repository, people and coding agents alike.

## Keep the web kit in step

Atelier ships twice: the gpui crate (`src/`) and the web kit (`web/`) for browsers and Tauri.
They are one design language, so a change to how Atelier looks lands in both, in the same pull
request.

- **Tokens, palettes, button paints, shaders or icons** (`src/theme.rs`, `src/button.rs`,
  `src/ambient/*.wgsl`, `assets/icons`, `assets/logos`): run
  `ATELIER_BLESS_WEB=1 cargo test --lib web` and commit `web/atelier/generated/`. `cargo test`
  fails while it is out of date. A new theme field the web needs goes into
  `atelier::web::theme_css` first.
- **A component's look or behaviour** (radius, fill, border, spacing, states, motion): make the
  same change to its `.at-*` class in `web/atelier/atelier.css`, and to `web/atelier/atelier.js`
  if it behaves differently. Class names follow the Rust names; see the table in
  [web/README.md](web/README.md).
- **A new component**: add its web counterpart and a sample in `web/showcase.html`, or say in the
  pull request why it stays native-only and list it under "Not yet" in `web/README.md`.
- **An ambient scene**: a new scene that its shader alone draws comes across through the
  generated module; add it to `web_scenes` in `src/ambient/mod.rs` if it is left out. One that
  needs a CPU simulation is listed as not yet ported in `web/README.md`.

Check the web side by opening `web/showcase.html` from a static server
(`python3 -m http.server --directory web`) in each direction and appearance
(`?theme=studio-light` and so on), and record user-visible changes under the web heading in
`CHANGELOG.md`.

## Other conventions

- Nothing outside `src/theme.rs` names a colour; the web kit names none outside its generated
  tokens.
- `cargo fmt`, `cargo clippy --all-targets` and `cargo test` pass before a push, as CI checks
  them on macOS, Windows and Linux.
- Add a `CHANGELOG.md` entry for anything an application would notice.
