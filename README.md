# Veritas

A terminal-native chat client (TUI) for OpenAI-compatible APIs, written in Rust.

Streams responses token-by-token over SSE against any `/v1/chat/completions`
endpoint — Ollama, llama.cpp server, OpenAI, OpenRouter, vLLM, etc.

![TUI mockup](Mockup%20of%20TUI.png)

## Features

- Token-by-token SSE streaming with Esc-to-cancel
- PgUp/PgDn, Ctrl-U/Ctrl-D, and mouse-wheel scrolling with follow-tail
- Shift+Enter for newlines, `/clear` to reset the conversation
- Context-usage gauge in the sidebar
- Single-file theme (`src/theme.rs`) for reskinning

## Configuration

Everything comes from environment variables:

| Variable | Default | Purpose |
|---|---|---|
| `OPENAI_BASE_URL` | `http://localhost:11434/v1` | API base (Ollama default shown) |
| `OPENAI_API_KEY` | *(empty)* | Bearer token; omitted from requests when unset |
| `OPENAI_MODEL` | `llama3.1` | Model id |

## Run

```sh
export OPENAI_BASE_URL="http://localhost:11434/v1"
export OPENAI_MODEL="llama3.1"
cargo run --release
```

## Keys

- `Enter` — send · `Shift+Enter` — newline
- `Esc` — stop streaming · `Ctrl-C` / `Ctrl-Q` — quit
- `PgUp`/`PgDn` or `Ctrl-U`/`Ctrl-D` or mouse wheel — scroll
