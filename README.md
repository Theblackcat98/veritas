# Veritas

A terminal-native chat client (TUI) for OpenAI-compatible APIs, written in Rust.

Streams responses token-by-token over SSE against any `/v1/chat/completions`
endpoint — Ollama, llama.cpp server, OpenAI, OpenRouter, vLLM, etc.

![TUI mockup](Mockup%20of%20TUI.png)

## Features

- Token-by-token SSE streaming with Esc-to-cancel
- PgUp/PgDn, Ctrl-U/Ctrl-D, and mouse-wheel scrolling with follow-tail
- Shift+Enter for newlines, `/clear` to reset the conversation
- Sessions: auto-saved JSON transcripts, Ctrl-S picker overlay, markdown
  export
- Live sidebar telemetry: RAM (sysinfo), AMD GPU VRAM + utilization (kernel
  DRM sysfs), context-window usage from server-reported tokens when available
- Slash commands with status-line feedback
- Single-file theme (`src/theme.rs`) for reskinning

## Configuration

Everything comes from environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `OPENAI_BASE_URL` | `http://localhost:11434/v1` | API base (Ollama default shown) |
| `OPENAI_API_KEY` | *(empty)* | Bearer token; omitted from requests when unset |
| `OPENAI_MODEL` | `jan-nano` | Model id |

Engine-owned parameters are not client config: requests omit `temperature`
unless you override it at runtime with `/temp`, and the context window is
probed from the engine (D009).

## Sidebar telemetry

- **RAM** — always shown (sysinfo).
- **VRAM / GPU %** — AMD GPUs (amdgpu) via the kernel's DRM sysfs, sampled
  every 500 ms; widgets are hidden on anything else. Never faked.
- **CTX** — used tokens come from the server's `usage` in stream chunks when
  it sends them; otherwise a chars/4 estimate is shown. The window total is
  probed from the engine at startup and on `/model` (Ollama `/api/show`);
  on engines without model-info endpoints the window shows `n/a` and no
  ratio gauge. The gauge shifts green → yellow (≥70%) → red (≥90%) as it
  fills.

## Commands

Type in the input box:

| Command | Effect |
|---|---|
| `/help` | List commands in the status line |
| `/sessions` | Open the session picker (same as Ctrl-S) |
| `/export <file>` | Write the transcript as markdown |
| `/model <id>` | Switch model (takes effect on next send; re-probes ctx window) |
| `/temp <0.0-2.0>` | Override sampling temperature — bare `/temp` shows the effective setting |
| `/clear` | Reset the conversation (next save starts a new session) |

## Sessions

Every exchange is auto-saved as JSON under
`$XDG_DATA_HOME/veritas/sessions` (default `~/.local/share/veritas/sessions`),
one file per conversation, named `sess-<unixsecs>.json`. Sessions are titled
from the first user message. Press `Ctrl-S` for the picker — ↑/↓ select,
Enter loads, Esc cancels.

## Run

```sh
export OPENAI_BASE_URL="http://localhost:11434/v1"
export OPENAI_MODEL="jan-nano"
cargo run --release
```

## Keys

- `Enter` — send · `Shift+Enter` — newline
- `Esc` — stop streaming · `Ctrl-S` — session picker · `Ctrl-C` / `Ctrl-Q` — quit
- `PgUp`/`PgDn` or `Ctrl-U`/`Ctrl-D` or mouse wheel — scroll
- In the picker: `↑`/`↓` select · `PgUp`/`PgDn` jump · `Enter` load · `Esc` cancel
