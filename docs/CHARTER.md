# Veritas Charter

> Veritas is a terminal-native chat client for OpenAI-compatible APIs with live
> system and context telemetry — mission control for talking to local LLMs.

This document is the app's identity. Features, dependencies, and UI changes must
serve it. When something feels like scope creep, this page is the tiebreaker.

## Goals — what we want out of the app

1. **Fast, distraction-free chat** with any `/v1/chat/completions` endpoint
   (Ollama, llama.cpp server, vLLM, OpenAI, OpenRouter…).
2. **Local-first**: the default target is Ollama on localhost. A remote API or
   account must never be required.
3. **Telemetry at a glance**: VRAM, RAM, GPU utilization, and context-window
   usage are always visible in the sidebar. This is a core identity feature,
   not decoration — the mockup gives it a third of the screen on purpose.
4. **Streaming-first UX**: tokens appear as they arrive; cancelling mid-stream
   keeps the partial output.
5. **Reskinnable in one file**: every color/style decision lives in
   `src/theme.rs`. Layout code never hardcodes colors.
6. **Small and boring**: single binary, flat modules, few dependencies,
   obvious code. A new contributor should understand the whole app in an
   afternoon.

## Non-goals — Veritas is NOT

- An **agent framework** — no tool calling, no function calling, no MCP,
  no multi-agent orchestration.
- An **IDE** — no tabs, no split panes beyond the fixed chat + sidebar layout,
  no file editing.
- A **config-heavy app** — env vars now; at most one small config file later,
  and only via a decision entry.
- A **cloud service** — no accounts, no sync, no outbound telemetry.
- **Extensible at runtime** — no plugin system, ever.

## Principles

| Principle        | Meaning in practice                                                 |
|------------------|---------------------------------------------------------------------|
| Terminal-native  | Looks at home in a terminal; works over SSH; no images              |
| Keyboard-first   | Every action reachable by key; mouse is scroll-only                  |
| Streaming-first  | Nothing waits for a full response; Esc always works                 |
| Local-first      | Works with zero network beyond the LLM endpoint                      |
| Boring tech      | Sync TUI loop + tokio for HTTP + mpsc channels; no actor frameworks  |

## Identity to preserve

- **Custom model, visible hardware.** Resist anything that
  dilutes this: tabs, agent loops, plugin ecosystems, config trees.
- Slash commands stay minimal and discoverable.
- Widgets come from ratatui; the sidebar tells the truth about the
  machine, or says "n/a" gracefully — it never fakes data once Phase 1 lands.
