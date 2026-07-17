# Interactive workbench shell

## Status
Accepted and implemented for the foundation slice.

## Problem
Equal-sized dashboard quadrants obscure the primary investigation flow, provide no command input, weakly communicate focus, and cannot save the active result independently of a full report.

## Goals
Provide a dominant timeline, contextual detail region, persistent input, visible focus, provider-neutral slash commands, command history, and structured active-view exports suitable for reuse.

## Non-goals
Conversational AI, general background jobs, mouse interaction, dynamic plugins, remote services, arbitrary terminal screenshots, full terminal emulation, SVG/PNG rendering, or extracting a standalone framework crate.

## User scenarios
An engineer opens the TUI, moves focus with Tab, enters `/findings`, inspects context, requests `/services`, saves the active content as Markdown or CSV, clears transient messages without deleting workspace state, and receives an explicit unavailable response for model commands when no provider exists.

## Functional requirements
The layout contains a header, dominant scrollable timeline, contextual detail panel, input row, and status row. Tab and Shift-Tab cycle timeline, context, and input; focused regions differ by border and color. `/` enters input from any region. Enter executes non-blank input and records bounded history. Up/Down traverse history while input is focused. Page Up/Down scroll the timeline by a page and Home/End move to its beginning/end for both text and table results. Escape clears/leaves input or returns focus to the timeline; Escape never terminates LoreMesh, even when pressed repeatedly. `q` quits only outside input, while `/quit` and `/exit` provide explicit command exits.

The parser recognizes `/help`, `/demo`, `/artifacts`, `/findings`, `/trace`, `/services`, `/model`, `/context`, `/compact`, `/clear`, `/save`, `/export`, `/shell`, and `/quit` with `/exit` as an alias in LoreMesh mode. Unknown commands and invalid arguments are errors without terminating the TUI. Results normally move focus to the timeline; `/shell` instead keeps the composer focused and changes it to shell input. `/help` renders complete syntax and examples suitable for humans and automated assistants. `/demo <table|chart|markdown|code|shell>` renders deterministic sample content without external files or execution. In LoreMesh mode `/quit` and `/exit` shut down cleanly. In shell mode `/exit` and Ctrl-D return to LoreMesh, Ctrl-C interrupts the child, and `/quit` exits the application. Model commands remain offline when no provider is configured.

`/save current --format <md|markdown-mermaid|markdown-d2|csv|html> [--output <path>]` sends the active structured view to an application handler. Markdown diagram formats include fenced source plus readable text. CSV requires a table. HTML is self-contained and escaped. PNG reports that an optional local renderer is not configured. Output paths follow the existing safe workspace-relative policy and never overwrite silently.

## Domain model
Shell state contains focus, input mode, input buffer, bounded LoreMesh history, timeline messages, selected view, and immutable `ViewContent`. `ViewContent` contains a title, paragraphs, optional rectangular table, and optional Mermaid/D2 source. Slash commands and save formats are typed enums. These are presentation/application protocol types, not LoreMesh domain entities.

## Interfaces
`ShellState::handle_key` performs pure transitions. `parse_command` returns a typed command or validation error. `CommandHandler::execute(command, active_view)` returns a content-safe response, `poll` yields streamed results, and `resize` reports terminal dimensions. Terminal rendering consumes shell state but performs no workspace I/O.

## Invariants
One region is focused; history and messages are bounded; command parsing never panics; input is not interpreted as shell syntax except after explicit `/shell` mode entry; TUI code does not access storage or network; active-view exports use structured content rather than screen scraping. Shell command text is not retained in LoreMesh history. Ordinary command results move focus to the timeline; streamed shell results preserve composer focus.

## Failure modes
Unknown command, malformed option, unavailable model/service, missing table, unsafe path, existing destination, rendering failure, terminal I/O failure, and storage I/O failure produce visible actionable messages without losing workspace state.

## Security and privacy implications
Ordinary commands never invoke a shell. The separately specified `/shell` boundary is never entered at startup and requires an explicit command each session. Its transcript is bounded, untrusted, excluded from LoreMesh history and exports, and sanitized for common terminal control sequences. The child retains the user's OS authority; its workspace working directory is not a sandbox. Network/model operations remain unavailable unless explicitly configured later. Saved views omit absolute paths and personal overlays by default. Messages and history must not contain source bodies or credentials. HTML escapes untrusted values.

## Observability requirements
The status row shows offline/service state. Command responses name operation and destination but not exported content. Future tracing records command names, outcome categories, and durations only.

## Acceptance criteria
Pure tests cover focus, input, history, parsing, clearing, active-view selection, multiline help, deterministic demos, and focus transfer after command results. The demo displays the layered layout. Local commands work without a model or network. Markdown, Mermaid, D2, CSV, and HTML saves are deterministic and path-safe. Windows, macOS, and Linux tests pass.

## Test strategy
Unit-test parser and shell transitions; property-test arbitrary command input does not panic; golden-test structured saves; CLI-test safe output handling; render with Ratatui's test backend without a real terminal; retain full offline integration tests.

## Deferred decisions
Async command execution, cancellation, completion popup, mouse support, PNG renderer choice, terminal screenshots, themes/configuration, framework extraction, provider selection, token estimation, and evidence-aware compaction.
