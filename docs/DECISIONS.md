# Decision Log

Append-only. Never edit or delete old entries — add a new entry that
supersedes it and mark the old one `superseded by Dxxx`.

## Template

```
## Dxxx — <title>
- **Date:** YYYY-MM-DD
- **Status:** accepted | superseded by Dxxx
- **Context:** why we had to choose
- **Decision:** what we chose
- **Consequences:** what this locks in / costs us
```

Rules:
1. One entry per choice, written when the choice is made (backfill freely
   when you notice a choice that was never logged).
2. Entries are immutable history. To change course, write a new entry that
   supersedes the old one.
3. Any new dependency, feature, or widget requires an entry here *and* a
   ROADMAP entry before implementation.

---

## D001 — Configuration via environment variables only
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Needed zero-friction config across Ollama/OpenAI/etc. for v0.
- **Decision:** `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` env vars;
  no config file.
- **Consequences:** No config parsing code; profile switching = shell exports.
  A TOML/JSON config requires a new decision.

## D002 — Sync TUI loop + tokio runtime + unbounded mpsc
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** The ratatui draw/event loop is synchronous; SSE needs async.
- **Decision:** Single sync event loop in `app::run`; HTTP streaming on tokio;
  events cross via `tokio::sync::mpsc::unbounded_channel`; cancellation =
  dropping the receiver so late sends are no-ops.
- **Consequences:** No async TUI framework. Partial output is preserved on
  cancel — keep that behavior in any refactor.

## D003 — Single-file theme; no hardcoded colors in ui.rs
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Mockup colors were structural placeholders; we want reskins
  without touching layout code.
- **Decision:** All colors/styles live behind `src/theme.rs` functions.
- **Consequences:** Every new UI element needs a theme token. One-off
  `Color::` uses in `ui.rs` are a review failure.

## D004 — Hand-rolled SSE parsing over reqwest bytes_stream
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Eventsource crates add a dependency for a simple `data:`-line
  protocol we only need one variant of.
- **Decision:** Parse `data:` lines manually in `provider.rs` with a 1 MiB
  buffer-growth guard.
- **Consequences:** provider.rs owns SSE quirks; OpenAI-compatible endpoints
  only. No auto-retry/reconnect — that needs its own decision.

## D005 — Sidebar telemetry mocked at Phase 0
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Layout and widgets mattered before data sources existed.
- **Decision:** `SysStats` generates fake waveforms; real NVML/sysinfo wiring
  is Phase 1.
- **Consequences:** Widget-facing data shapes must not change when real data
  lands (ratios 0..1, history `Vec<u64>`).

## D006 — Lightweight markdown rendering for agent messages
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Scannable, readable model output matters in a chat client
  (ROADMAP Phase 3).
- **Decision:** Render agent message content as lightweight markdown: fenced
  code blocks, inline code, bold/italic — explicitly **not** full CommonMark.
  The rendering library is a separate choice and gets its own entry before
  implementation (rule 3).
- **Consequences:** Plain-text rendering stays until the library decision
  lands; the full-markdown rejection in ROADMAP applies.

## D007 — u16 scroll clamp (ratatui 0.29 overflow guard)
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Paragraph scroll computes `area.height + scroll.y` in u16 and
  overflows near `u16::MAX`.
- **Decision:** Clamp scroll to `u16::MAX - height - 1`; `MAX_SCROLL` const
  bounds manual scrolling.
- **Consequences:** Revisit if a ratatui release fixes the overflow
  internally.

## D008 — RAM telemetry via the `sysinfo` crate
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Phase 1 requires real RAM numbers; nothing in the dep tree
  reads memory info, and hand-rolled `/proc` parsing would be Linux-only.
- **Decision:** Add `sysinfo` for total/used system RAM.
- **Consequences:** One more dependency; refresh cost is paid inline in the
  TUI loop (see D010). RAM is always available, so the RAM gauge never hides.

## D009 — NVIDIA telemetry via `nvml-wrapper`; hide widgets when absent
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** VRAM and GPU utilization have no portable API; NVML is the
  de-facto standard for NVIDIA.
- **Decision:** Add `nvml-wrapper` (safe NVML bindings). If `Nvml::init()`
  fails (no NVIDIA driver/GPU), the VRAM gauge and GPU sparkline are not
  rendered at all — per the charter: no fake data, no crash.
- **Consequences:** AMD/Intel GPU users get no VRAM/GPU telemetry; adding
  another vendor needs a new decision.

## D010 — Telemetry sampled inline in the TUI event loop
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Stats need periodic refresh; the loop already ticks at 100 ms
  and D002 keeps the TUI loop synchronous.
- **Decision:** A `sysmon::Sampler` (sysinfo + NVML handles) lives in
  `app::run` and samples every 500 ms. No sampler thread, no locks, no
  channels.
- **Consequences:** A slow sample would jank the loop; acceptable because
  `refresh_memory` and NVML queries are sub-millisecond. `SysStats` now holds
  raw counters (bytes, tokens) instead of precomputed ratios so titles can
  show real numbers — this supersedes the shape lock in D005; `gpu_history`
  stays `Vec<u64>` 0..100.

## D011 — Context window: `OPENAI_CTX_SIZE`; usage from stream, chars/4 fallback
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** The CTX gauge needs a window total that the chat API does not
  expose; some servers include token `usage` in stream chunks.
- **Decision:** Window total comes from `OPENAI_CTX_SIZE` (default 8192).
  Used tokens come from `usage.prompt_tokens + usage.completion_tokens` when
  the server sends it (surfaced as a `Usage` stream event); otherwise the
  chars/4 estimate stands. The request body is unchanged — usage is only
  parsed opportunistically, so strict endpoints are not broken.
- **Consequences:** A wrong `OPENAI_CTX_SIZE` produces an honest-but-wrong
  gauge; documented in README. Endpoints that never send usage stay on
  estimates.

## D012 — Runtime model params: temperature config + `/model`, `/temp` overrides
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** `provider.rs` hardcoded temperature 0.7 and the sidebar showed
  a fake temp; ROADMAP Phase 1 wants real params from config.
- **Decision:** Temperature comes from `OPENAI_TEMPERATURE` (default 0.7).
  `/model <id>` and `/temp <f>` mutate the live config; the sidebar table
  reflects current values. Slash commands never enter the chat history.
- **Consequences:** Extends D001's env-var surface. `/model` does not
  validate the id against the server — bad ids surface as errors on next
  send.
