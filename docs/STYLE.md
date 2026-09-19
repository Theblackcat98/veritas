# Veritas visual style

**Working direction:** Quiet Mission Control  
**Purpose:** Shared visual language for the Veritas terminal UI and its future theme presets.

> Veritas is a calm, high-signal terminal instrument panel for talking to local models: truthful telemetry, focused conversation, restrained command-console chrome.

This document describes reusable roles and relationships, not one-off widget styling. The Rust implementation should keep color and style values in `src/theme.rs`; layout and content code should consume semantic theme tokens.

## Identity

Veritas should feel like reliable local mission control rather than a generic chat app or a neon hacker dashboard.

- **Dark and sparse:** thin borders, compact spacing, and little decorative chrome.
- **High signal:** color communicates role, state, or focus; it is not decoration.
- **Conversation first:** the transcript is the visual center; telemetry supports it without competing.
- **Keyboard native:** actions and key hints are visible, compact, and consistent.
- **Truthful:** unavailable telemetry is muted or marked `n/a`; it is never represented by fake activity.
- **Reskinnable:** all colors and reusable styles are theme tokens, not literals in `ui.rs`.

## Palette

The palette is intentionally restrained. Cyan/teal is the structural accent. Blue and mint identify the two conversation roles. Amber is reserved for live activity and pressure. Red is reserved for errors and danger.

These values are the reference palette for the visual preview. The Rust theme may use terminal-safe equivalents where necessary, while preserving the same relationships.

| Token | Reference color | Role |
|---|---|---|
| `canvas` | `#080C10` | Main terminal background |
| `panel` | `#0F171D` | Raised application panels |
| `modal` | `#111D24` | Session/help overlay surface |
| `code_surface` | `#16212A` | Code and preformatted content |
| `border` | `#33434E` | Quiet panel and divider borders |
| `border_focus` | `#22D3EE` | Focused input, active modal, selected structure |
| `primary` | `#22D3EE` | Titles, navigation, structural accent |
| `teal` | `#2DD4BF` | Secondary accent and telemetry activity |
| `text` | `#E6EDF3` | Main readable content |
| `muted` | `#94A3B8` | Metadata, descriptions, secondary text |
| `faint` | `#64748B` | Hints, separators, disabled/supporting text |
| `user` | `#7DD3FC` | User label and user message accent |
| `agent` | `#A7F3D0` | Agent label and agent message accent |
| `activity` | `#FBBF24` | Streaming, active work, spinner |
| `warning` | `#F59E0B` | Recoverable warning and context pressure |
| `error` | `#F87171` | Failed commands, connection errors, danger |
| `success` | `#34D399` | Confirmed successful operation |
| `link` | `#7DD3FC` | Links and navigable references |

### Color discipline

- Use `primary` for application structure, not every important piece of content.
- Use `user` and `agent` only for conversation identity.
- Use `activity` only when something is happening now.
- Use `warning` and `error` only for state that needs attention.
- Prefer modifiers such as bold, italic, and underline before introducing another color.
- Markdown hierarchy may use nearby accent shades, but should remain inside this palette.
- Avoid rainbow styling, gradients, glow effects, and decorative backgrounds.

## Reusable visual roles

These are the roles that should exist in `Theme`. Names are conceptual; the Rust API can use the project’s existing naming conventions.

### Surfaces

| Role | Use |
|---|---|
| `canvas()` | The base application surface |
| `panel()` | Sidebar and primary application regions |
| `modal_surface()` | Opaque surface for centered overlays |
| `code_surface()` | Background for code blocks and preformatted text |

### Chrome

| Role | Use |
|---|---|
| `border()` | Quiet borders around ordinary panels |
| `border_focus()` | Focused input, active modal, or selected structural element |
| `title()` | Panel and modal titles; bold primary accent |
| `section_title()` | Interior headings such as `KEYBOARD` and `COMMANDS` |
| `separator()` | Transcript rules and low-emphasis dividers |

### Text

| Role | Use |
|---|---|
| `text()` | Main readable text |
| `muted()` | Descriptions, metadata, and secondary content |
| `faint()` | Hints, ages, counts, disabled states, and footer copy |
| `hint()` | Short instructions attached to controls |

### Interaction

| Role | Use |
|---|---|
| `key_hint()` | Keyboard names such as `Enter`, `Ctrl-S`, and `Esc` |
| `command_hint()` | Slash commands such as `/help` and `/sessions` |
| `selected_row()` | Filled selection state in lists and pickers |
| `empty_state()` | Empty content message plus its next-action hint |
| `activity()` | Streaming and currently active work |

### Conversation

| Role | Use |
|---|---|
| `user_label()` | User message marker and label |
| `user_text()` | User message body |
| `agent_label()` | Agent message marker and label |
| `agent_text()` | Agent message body |

The conversation colors should be recognizable but not overpower the content. Labels can be bold; message bodies should remain comfortable to read over long sessions.

### State

| Role | Use |
|---|---|
| `success()` | Saved, loaded, or completed actions |
| `warning()` | Context pressure and recoverable problems |
| `error()` | Request failures and invalid commands |
| `activity()` | Streaming shimmer, spinner, and active operations |

### Telemetry

Telemetry should look like instrumentation, not decoration.

| Role | Use |
|---|---|
| `gauge_vram()` | VRAM usage; primary/teal family |
| `gauge_ram()` | RAM usage; user/blue family |
| `gauge_gpu()` | GPU activity; teal family |
| `gauge_ctx(ratio)` | Context usage: success below 70%, warning at 70%, error at 90% |
| `telemetry_value()` | Numeric values and units |
| `telemetry_unavailable()` | `n/a` and unavailable source labels |

Unavailable sources should be quiet and explicit. Do not show placeholder waveforms or imply data exists when it does not.

### Markdown

Markdown is content styling, not application chrome. Keep its colors close to the main palette:

| Markdown role | Treatment |
|---|---|
| H1 | `primary`, bold |
| H2 | `teal`, bold |
| H3 | `activity` or a muted primary variant, bold/italic |
| Inline/fenced code | `link`/cyan text on `code_surface` |
| Link | `link`, underlined |
| Blockquote | muted green/mint, italic |
| Bold | main text plus bold; optional restrained accent |
| Italic | main text plus italic; optional restrained accent |
| Horizontal rule | `faint` separator |

## Shared modal language

The session picker and help overlay are instances of the same modal component language.

1. Center the modal with content-appropriate dimensions.
2. Punch out the underlying buffer with `Clear`.
3. Use the same opaque `modal_surface()` for both overlays.
4. Use the same border, title, padding, and footer spacing.
5. Make the active modal border more visible than ordinary panel borders.
6. Keep the title short; put keyboard instructions in the footer.
7. Use the same `key_hint()`, `muted()`, and `faint()` roles in both dialogs.
8. Keep modal content keyboard-first and never require mouse interaction.

### Session picker

The session list should read as a calm, scannable index:

```text
╭─ Sessions ─────────────────────────────────────╮
│                                                │
│ › Project notes                 12 msgs · 2m ago│
│   Research chat                 28 msgs · 1h ago│
│                                                │
│             ↑↓ navigate · Enter load · Esc close│
╰────────────────────────────────────────────────╯
```

- Use a single selection marker such as `›`, or rely on the list highlight symbol.
- Session titles use primary readable text.
- Counts and ages use `faint()` or `muted()`.
- The selected row uses `selected_row()` with bold text.
- Empty state explains what to do next without pretending sessions exist.
- Do not use markdown heading/code tokens for session titles or metadata.

### Help overlay

The help dialog should have a clear two-level hierarchy:

```text
╭─ Help ─────────────────────────────────────────╮
│ KEYBOARD                                       │
│ Enter          Send message                    │
│ Shift+Enter    Newline in input               │
│ Esc            Stop stream / close overlay     │
│                                                │
│ COMMANDS                                       │
│ /help          Show this help                  │
│ /sessions      Open session picker             │
│ /clear         Clear transcript                │
│                                                │
│             Esc close                          │
╰────────────────────────────────────────────────╯
```

- The outer `Help` title uses `title()`.
- `KEYBOARD` and `COMMANDS` use `section_title()`.
- Key names use `key_hint()`.
- Slash commands use `command_hint()`.
- Descriptions use `muted()`.
- The footer uses `faint()` or `hint()`.
- Do not repeat the close instruction in both the border title and footer.

## Application component guidance

### Panels and input

- Ordinary panels use a quiet one-cell border.
- The input panel becomes more visible when focused, using `border_focus()` rather than a new color.
- Titles are short and structural: `Chat`, `Input`, `Model`, `Sessions`, `Help`.
- Action instructions belong in compact hints, not long border titles.

### Transcript

- Conversation is the visual center of the application.
- User and agent markers provide orientation between messages.
- Separators are faint and should not look like additional content.
- Streaming uses the activity color only for the active state and cursor/spinner.
- Long reading sessions should remain comfortable at a glance; avoid saturated body text.

### Sidebar

- The sidebar is instrumentation: compact labels, clear values, restrained gauges.
- Keep the sidebar’s structural title in the primary accent family.
- Numeric values and units should be more readable than their labels.
- Gauges communicate status through the telemetry state scale, especially CTX.

## Implementation notes

- Keep all color/style construction in `src/theme.rs`, per D003.
- `ui.rs` should not contain direct `Color::...` values or one-off `Style::default()` color decisions.
- Replace picker-specific styling that is reused elsewhere with semantic roles such as `muted()`, `key_hint()`, and `modal_surface()`.
- Keep widget choice boring and built-in: `Block`, `List`, `Paragraph`, `Gauge`, `Table`, `Clear`, and existing text widgets are sufficient.
- The browser preview at `docs/style-preview.html` is a visual reference, not a runtime dependency or a second source of application behavior.
