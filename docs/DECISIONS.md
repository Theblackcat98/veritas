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

## D009 — All model info is gathered from the inference engine
- **Date:** 2026-09-18
- **Status:** accepted
- **Context:** Model parameters — temperature, context window, model id —
  are owned by the inference engine. Client-side copies (extra env vars or
  hardcoded values) either override the engine's own settings or make the
  sidebar lie.
- **Decision:** No client config for model params; config stays the D001
  trio. Requests omit `temperature` unless the user overrides it at runtime
  via `/temp <f>` (bare `/temp` reports the effective setting). The context
  window is probed from the engine at startup and on `/model` (Ollama
  `POST {root}/api/show`: `num_ctx` if set, else `context_length`); used
  tokens come from stream `usage` when the server sends it, else the
  chars/4 estimate. `/model <id>` mutates live state and re-probes.
  Anything unknowable renders `n/a` (rule 9). Slash commands never enter
  chat history.
- **Consequences:** provider.rs makes one Ollama-specific metadata call,
  widening D004's chat-completions-only scope deliberately; other engines
  fall back to `n/a`. No retry logic on the probe; `/model` does not
  validate the id against the server — bad ids surface on next send.

## D010 — Telemetry sampled inline in the TUI event loop
- **Date:** 2026-09-17
- **Status:** accepted
- **Context:** Stats need periodic refresh; the loop already ticks at 100 ms
  and D002 keeps the TUI loop synchronous.
- **Decision:** A `sysmon::Sampler` (sysinfo handle + kernel sysfs reads)
  lives in `app::run` and samples every 500 ms. No sampler thread, no
  locks, no channels.
- **Consequences:** A slow sample would jank the loop; acceptable because
  `refresh_memory` and sysfs reads are sub-millisecond. `SysStats` holds
  raw counters (bytes, tokens) instead of precomputed ratios so titles can
  show real numbers — this supersedes the shape lock in D005;
  `gpu_history` stays `Vec<u64>` 0..100.

## D011 — Follow-tail via pre-wrapped row counts; `textwrap` for wrapping
- **Date:** 2026-09-18
- **Status:** accepted
- **Context:** ratatui 0.29's Paragraph does not saturate `scroll.y` — an
  offset past the content paints blank. Follow mode used a `u16::MAX`
  sentinel (kept in bounds by D007's clamp), so the transcript went blank
  whenever follow was active; streaming made it look like chat disappeared.
- **Decision:** The transcript pre-wraps every logical line with
  `textwrap` (already a dependency since the initial commit, previously
  unused), so the exact rendered row count is known. Follow pins scroll to
  `total_rows - view_height`; manual scroll clamps to the same bound;
  `MAX_SCROLL` doubles as the u16 overflow guard (D007). Paragraph wrap
  mode removed.
- **Consequences:** Re-wrapping happens per frame — fine at chat sizes;
  cache per message if profiling ever demands it. Content beyond
  `MAX_SCROLL` rows (60k) is out of reach, an accepted edge. TestBackend
  rendering tests lock the follow/scroll behavior in.

## D012 — Sessions: JSON files, std-only time, auto-save on stream end
- **Date:** 2026-09-18
- **Status:** accepted
- **Context:** Phase 2 requires durable transcripts. Time formatting is the
  only reason a session store would want a date crate, and the picker only
  needs coarse relative ages.
- **Decision:** One JSON file per conversation under
  `$XDG_DATA_HOME/veritas/sessions` (else `~/.local/share/...`), named
  `sess-<unixsecs>.json` with a `version` field for future migrations.
  Writes are atomic (tmp + rename). Timestamps are std unix seconds — no
  chrono; the UI renders `Xm/h/d ago`. Auto-title = first user message,
  whitespace-collapsed, ≤40 chars. Saves fire after each finished or
  cancelled stream and before the picker opens; a message-count dirty
  check skips no-op writes. `/clear` drops the session identity so the
  next save starts a new file instead of overwriting the cleared one.
  Corrupt files are skipped by the lister, never repaired.
- **Consequences:** No new dependencies (serde/serde_json already in the
  tree). `list` reads every file fully — fine at human session counts.
  Timestamp collision in ids is theoretically possible across hosts;
  irrelevant for a single-machine TUI.

## D013 — Session picker: centered Clear+List modal, Ctrl-S or /sessions
- **Date:** 2026-09-18
- **Status:** accepted
- **Context:** Phase 2 requires a keyboard session chooser. The ROADMAP
  widget inventory pre-approved `Clear` for overlay backgrounds.
- **Decision:** Ctrl-S (and `/sessions`) opens a centered 60%×60% modal:
  `Clear` punches out the buffer, a ratatui `List` shows title · msgs ·
  age, newest first. Modal routing: while open, every key goes to the
  picker (↑/↓ ±1, PgUp/PgDn ±10, wrap-around; Enter loads; Esc cancels);
  typing cannot fall through to the input. Opening while streaming is
  refused with a status hint instead of saving a partial exchange. The
  picker never touches disk itself — it renders `session::list` output
  and `confirm_load` applies `session::load`.
- **Consequences:** No new widget crate (rule 8). Overlay rendering is
  untested-below-TestBackend like the rest of ui.rs — rendering tests
  cover punch-through, empty state, and key routing. No delete/rename
  action in the picker: add via a new decision if wanted.

## D014 — `/export <file>` writes hand-rolled markdown
- **Date:** 2026-09-18
- **Status:** accepted
- **Context:** Phase 2 export. A markdown crate would import a parser to
  do a serializer's job.
- **Decision:** `session::export_markdown` emits `# <title>` then
  `## You` / `## Agent` sections with message content verbatim. Failures
  (unwritable path) surface in the status line, never panic.
- **Consequences:** No dependency. Content is not escaped — a message
  containing `##` renders as a heading in downstream viewers; accepted
  because the export is a faithful transcript, not sanitized prose.
