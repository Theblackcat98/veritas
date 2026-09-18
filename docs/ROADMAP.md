# Roadmap

The only source of approved features. Anything not listed here is **out of
scope** until it is added here *together with* a `docs/DECISIONS.md` entry.

Status: ✅ done · 🔜 next · 🧭 later · 🚫 rejected

## Phase 0 — Foundation ✅

- SSE token streaming against `/v1/chat/completions` (`provider.rs`)
- Esc-to-cancel, keeping partial output
- Scrolling: PgUp/PgDn, Ctrl-U/Ctrl-D, mouse wheel, follow-tail
- Shift+Enter newlines, `/clear`
- Sidebar skeleton: header, model table, VRAM/RAM gauges, GPU sparkline,
  CTX gauge (mocked data)
- Single-file theme, env-var config

## Phase 1 — Make the mockup honest ✅

The sidebar showed fake data. Now real sources with graceful fallbacks
(show `n/a`, hide widgets, never crash):

- RAM via `sysinfo` crate ✅
- VRAM + GPU% via amdgpu DRM sysfs (AMD); hide widgets when no GPU reports
  data ✅
- Real context size: parse `usage` from stream chunks when the server sends
  it; keep the chars/4 estimate as fallback ✅
- Real model params in the sidebar table — temperature is engine-owned
  (omitted from requests unless `/temp` overrides), ctx window probed from
  the engine ✅
- `/help`, `/model <id>`, `/temp <f>` slash commands with status-line
  feedback ✅
- CTX gauge color shift (green → yellow → red) as the window fills ✅

## Phase 2 — Sessions 🔜

- Save/load transcripts as JSON under XDG data dir
  (`~/.local/share/veritas/sessions`)
- Auto-title a session from the first user message
- Keyboard session-picker overlay
- `/export <file>` writes the transcript as markdown

## Phase 3 — Reading experience 🧭

- Lightweight markdown for agent messages: fenced code blocks, inline code,
  bold/italic — explicitly **not** full CommonMark
- Keyboard copy of code-block contents
- Readability pass on the theme (spacing, label styling)

## Phase 4 — Polish & release 🧭

- Theme presets selectable via env var (still one theme file)
- `?` help overlay listing keybindings
- CI: build + clippy
- `--version` flag, README screenshots

## Explicitly rejected 🚫

| Idea                          | Why rejected                                    |
|-------------------------------|-------------------------------------------------|
| Tool calling / agents / MCP   | Violates charter: Veritas is a chat client      |
| Plugin system                 | Complexity trap; themes + forks cover it        |
| Tabs / multi-session UI       | One conversation at a time is the identity      |
| Full markdown/HTML rendering  | Terminal-native means light markup only         |
| 50-key config file            | Env vars now; one TOML only if a decision demands |

## Widget inventory

| Widget (source)              | Used for                    | Status                        |
|------------------------------|-----------------------------|-------------------------------|
| Paragraph (ratatui)          | Transcript, status/spinner  | ✅                            |
| TextArea (tui-textarea)      | Input                       | ✅                            |
| Gauge (ratatui)              | VRAM, RAM, CTX              | ✅ real (P1)                  |
| Sparkline (ratatui)          | GPU % history               | ✅ real (P1)                  |
| Table (ratatui)              | Model params                | ✅ real params (P1)           |
| Scrollbar (ratatui)          | Transcript position         | ✅                            |
| Clear widget (ratatui)       | Overlay backgrounds         | 🔜 Phase 2/4                  |

New widgets need a DECISIONS entry (see AGENTS.md rule 8).

## Target keybindings

| Key                          | Action                     | Status |
|------------------------------|----------------------------|--------|
| Enter / Shift+Enter          | Send / newline             | ✅     |
| Esc                          | Cancel stream              | ✅     |
| Ctrl-C / Ctrl-Q              | Quit                       | ✅     |
| PgUp/PgDn, Ctrl-U/D, wheel   | Scroll                     | ✅     |
| `/help`                      | Command list in status bar | ✅     |
| `?`                          | Help overlay               | 🔜     |
| `/model`, `/temp`            | Set params                 | ✅     |
| Ctrl-S                       | Save session               | 🔜 P2  |
