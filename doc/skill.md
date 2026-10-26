# Using the Skill

`skills/atelier-gpui` is an Agent Skill in the shared `SKILL.md` format, read by both Claude Code
and Codex. Link it into each agent's skill directory so updates to this repository reach both.

macOS / Linux:

```bash
ln -s "$PWD/skills/atelier-gpui" ~/.claude/skills/atelier-gpui
ln -s "$PWD/skills/atelier-gpui" ~/.agents/skills/atelier-gpui
```

Windows (PowerShell, Developer Mode or an elevated shell for symbolic links):

```powershell
New-Item -ItemType SymbolicLink -Path "$HOME\.claude\skills\atelier-gpui" -Target "$PWD\skills\atelier-gpui"
New-Item -ItemType SymbolicLink -Path "$HOME\.agents\skills\atelier-gpui" -Target "$PWD\skills\atelier-gpui"
```

`~/.agents/skills` is the user-level directory Codex reads; use `~/.codex/skills` instead on
Codex versions that predate it. To scope the Skill to one project, link it into that project's
`.claude/skills/` and `.agents/skills/` instead.

The Skill triggers on gpui UI work that uses the `atelier` crate, on adding an AI assistant
panel or chat to a gpui app, and on questions about the design candidates or tokens. It points
the agent at the crate source and `examples/showcase.rs` rather than restating the API, so it
stays correct as the crate changes; keep `references/*.md` in step when an API changes.
