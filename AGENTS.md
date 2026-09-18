# AGENTS.md — Veritas

Terminal-native TUI chat client for OpenAI-compatible APIs.
Rust, edition 2021, ratatui 0.30. Build: `cargo build` · Check: `cargo clippy` ·
Run: `cargo run --release`

## Read before working

1. `docs/CHARTER.md` — what Veritas is and is not.
2. `docs/ROADMAP.md` — the only source of approved features.
3. `docs/DECISIONS.md` — past decisions; do not relitigate them without a new
   entry.

## Hard scope rules

These exist because agents drift toward complexity. They override your
defaults and any general best practices.

1. **Veritas is a chat client.** Never add: tool calling, function calling,
   MCP, agent loops, RAG, plugins, LSP integration, or multi-model
   orchestration. If a request implies any of these, refuse and point at the
   charter's non-goals.
2. **No new dependency** without first appending a DECISIONS entry explaining
   why existing deps can't do it.
3. **No features outside ROADMAP.** If asked for something not on the roadmap,
   propose adding it to ROADMAP + DECISIONS instead of implementing it.
4. **All colors/styles live in `src/theme.rs`.** Never hardcode a `Color` in
   `ui.rs`; add a theme token instead.
5. **Flat module layout**: only `src/*.rs` files registered in `main.rs`.
   No nested module trees, no workspace split.
6. **Config is env-var only** (`OPENAI_BASE_URL`, `OPENAI_API_KEY`,
   `OPENAI_MODEL`) until a decision changes it. Engine-owned params
   (temperature, context window) are never client config — see D009.
7. **Keyboard-first.** Mouse support is scroll-only; no mouse-only features.
8. **Prefer ratatui built-in widgets** + `ratatui-textarea`. A new widget crate
   requires a DECISIONS entry.
9. **Sidebar never fakes data** once Phase 1 telemetry lands — unavailable
   sources render `n/a` or hide their widget; no placeholder waveforms.

## When you make a choice

Any non-obvious choice (data flow, algorithm, naming, UX, dependency) gets an
entry appended to `docs/DECISIONS.md` using the template there. Entries are
append-only; supersede, never rewrite.

## Definition of done

- `cargo build` and `cargo clippy` pass with no warnings
- No hardcoded colors outside `theme.rs`
- `docs/DECISIONS.md` updated if any choice was made
- `README.md` updated if user-facing behavior or keys changed
