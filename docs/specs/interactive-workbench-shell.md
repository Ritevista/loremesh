# Interactive workbench shell

## Status
Accepted for the next foundation slice.

## Problem
Equal-sized dashboard quadrants obscure the primary investigation flow, provide no command input, weakly communicate focus, and cannot save the active result independently of a full report.

## Goals
Provide a dominant timeline, contextual detail region, persistent input, visible focus, provider-neutral slash commands, command history, and structured active-view exports suitable for reuse.

## Non-goals
Conversational AI, asynchronous jobs, mouse interaction, dynamic plugins, remote services, arbitrary terminal screenshots, SVG/PNG rendering, or extracting a standalone framework crate.

## User scenarios
An engineer opens the TUI, moves focus with Tab, enters `/findings`, inspects context, requests `/services`, saves the active content as Markdown or CSV, clears transient messages without deleting workspace state, and receives an explicit unavailable response for model commands when no provider exists.

## Functional requirements
The layout contains a header, dominant scrollable timeline, contextual detail panel, input row, and status row. Tab and Shift-Tab cycle timeline, context, and input; focused regions differ by border and color. `/` enters input from any region. Enter executes non-blank input and records bounded history. Up/Down traverse history while input is focused. Escape leaves input or quits after a second Escape; `q` quits only outside input.

The parser recognizes `/help`, `/demo`, `/artifacts`, `/findings`, `/trace`, `/services`, `/model`, `/context`, `/compact`, `/clear`, `/save`, `/export`, and `/quit` with `/exit` as an alias. Unknown commands and invalid arguments are errors without terminating the TUI. Every completed command replaces or selects meaningful investigation-timeline content and moves focus from input to the timeline. `/help` renders a stable multiline reference containing complete syntax and examples suitable for humans and automated assistants. `/demo <table|chart|markdown|code|shell>` renders deterministic sample content without external files or command execution. `/clear` clears transient timeline messages only. `/quit` and `/exit` perform a clean terminal shutdown. `/model`, `/context`, and `/compact` explain that no model is configured; no network operation occurs.

`/save current --format <md|markdown-mermaid|markdown-d2|csv|html> [--output <path>]` sends the active structured view to an application handler. Markdown diagram formats include fenced source plus readable text. CSV requires a table. HTML is self-contained and escaped. PNG reports that an optional local renderer is not configured. Output paths follow the existing safe workspace-relative policy and never overwrite silently.

## Domain model
Shell state contains focus, input buffer, bounded history, timeline messages, selected view, and immutable `ViewContent`. `ViewContent` contains a title, paragraphs, optional rectangular table, and optional Mermaid/D2 source. Slash commands and save formats are typed enums. These are presentation/application protocol types, not LoreMesh domain entities.

## Interfaces
`ShellState::handle_key` performs pure transitions. `parse_command` returns a typed command or validation error. `CommandHandler::execute(command, active_view)` returns a content-safe response. Terminal rendering consumes shell state but performs no workspace I/O.

## Invariants
One region is focused; history and messages are bounded; command parsing never panics; input is not interpreted as shell syntax; TUI code does not access storage or network; active-view exports use structured content rather than screen scraping. Shell command text is not retained in history. Command results and errors become upper-window content, and returning content moves focus to the timeline.

## Failure modes
Unknown command, malformed option, unavailable model/service, missing table, unsafe path, existing destination, rendering failure, terminal I/O failure, and storage I/O failure produce visible actionable messages without losing workspace state.

## Security and privacy implications
Ordinary commands never invoke a shell. The separately specified `/shell` boundary is disabled at startup and requires explicit per-session enablement. Network/model operations remain unavailable unless explicitly configured later. Saved views omit absolute paths and personal overlays by default. Messages and history must not contain source bodies or credentials. HTML escapes untrusted values.

## Observability requirements
The status row shows offline/service state. Command responses name operation and destination but not exported content. Future tracing records command names, outcome categories, and durations only.

## Acceptance criteria
Pure tests cover focus, input, history, parsing, clearing, active-view selection, multiline help, deterministic demos, and focus transfer after command results. The demo displays the layered layout. Local commands work without a model or network. Markdown, Mermaid, D2, CSV, and HTML saves are deterministic and path-safe. Windows, macOS, and Linux tests pass.

## Test strategy
Unit-test parser and shell transitions; property-test arbitrary command input does not panic; golden-test structured saves; CLI-test safe output handling; render with Ratatui's test backend without a real terminal; retain full offline integration tests.

## Deferred decisions
Async command execution, cancellation, completion popup, mouse support, PNG renderer choice, terminal screenshots, themes/configuration, framework extraction, provider selection, token estimation, and evidence-aware compaction.
