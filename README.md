# Veritas

A terminal-native chat client (TUI) for OpenAI-compatible APIs, written in Rust.

Streams responses token-by-token over SSE against any `/v1/chat/completions`
endpoint — Ollama, llama.cpp server, OpenAI, OpenRouter, vLLM, etc.

![TUI mockup](Mockup%20of%20TUI.png)

## Features

- Token-by-token SSE streaming with Esc-to-cancel
- PgUp/PgDn, Ctrl-U/Ctrl-D, and mouse-wheel scrolling with follow-tail
- Shift+Enter for newlines, `/clear` to reset the conversation
- Live sidebar telemetry: RAM (sysinfo), VRAM + GPU% (NVIDIA via NVML),
  context-window usage from server-reported tokens when available
- Slash commands with status-line feedback
- Single-file theme (`src/theme.rs`) for reskinning

## Configuration

Everything comes from environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `OPENAI_BASE_URL` | `http://localhost:11434/v1` | API base (Ollama default shown) |
| `OPENAI_API_KEY` | *(empty)* | Bearer token; omitted from requests when unset |
| `OPENAI_MODEL` | `jan-nano` | Model id |
| `OPENAI_TEMPERATURE` | `0.7` | Sampling temperature (0.0–2.0) |
| `OPENAI_CTX_SIZE` | `8192` | Context window in tokens, for the CTX gauge |

## Sidebar telemetry

- **RAM** — always shown (sysinfo).
- **VRAM / GPU %** — shown only when an NVIDIA GPU is present (NVML);
  the widgets are hidden otherwise. Never faked.
- **CTX** — used tokens come from the server's `usage` in stream chunks when
  it sends them; otherwise a chars/4 estimate is shown. The window total is
  `OPENAI_CTX_SIZE`, so set it to match your model/server. The gauge shifts
  green → yellow (≥70%) → red (≥90%) as it fills.

## Commands

Type in the input box:

| Command | Effect |
|---|---|
| `/help` | List commands in the status line |
| `/model <id>` | Switch model (takes effect on next send) |
| `/temp <0.0-2.0>` | Set sampling temperature |
| `/clear` | Reset the conversation |

## Run

```sh
export OPENAI_BASE_URL="http://localhost:11434/v1"
export OPENAI_MODEL="jan-nano"
cargo run --release
```

## Keys

- `Enter` — send · `Shift+Enter` — newline
- `Esc` — stop streaming · `Ctrl-C` / `Ctrl-Q` — quit
- `PgUp`/`PgDn` or `Ctrl-U`/`Ctrl-D` or mouse wheel — scroll
