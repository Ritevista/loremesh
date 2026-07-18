# Data workbench

## Status

Accepted. The initial synchronous grid, CSV, chart, bounded one-shot runner, and persistent interactive shell are implemented; richer navigation remains deferred.

## Problem

Investigations produce tables and numeric series that need interactive inspection, local CSV interchange, charts, and occasional processing with installed command-line tools. The current TUI renders static tables and has no explicit local-execution trust boundary.

## Goals

Provide a reusable data grid, renderer-neutral multi-series chart models, responsive and color-accessible terminal charts, safe local CSV load/save, refresh from the same file, and explicit bounded local shell execution.

## Non-goals

Excel formulas, cell editing, XLSX compatibility, arbitrary SQL, background schedulers, remote data sources, automatic command execution, command-output parsing as evidence, graphical PNG/SVG chart rendering, or a portable operating-system sandbox.

## User scenarios

A user loads a workspace-relative CSV, searches all visible cells, filters and sorts by named columns, hides irrelevant columns, refreshes after an external tool updates the file, and saves the current result to another CSV. The user renders a numeric column as bar, horizontal-bar, line, or pie data. The user enters a local shell, runs ordinary commands with persistent directory and environment state, reviews streamed output, and returns to LoreMesh without closing the TUI.

## Functional requirements

The grid supports row selection, vertical and horizontal navigation, stable ascending/descending sort, case-insensitive global search, per-column contains filters, column visibility, reset, and a visible count of matching and total rows. Operations compose deterministically and preserve the immutable loaded rows. Unknown columns and non-rectangular input are rejected actionably.

`/table load <path>` loads UTF-8 CSV inside the workspace; `/table refresh` reloads its original path; `/table save <path>` writes the current visible rows and columns without overwriting; `/table sort <column> <asc|desc>`, `/table filter <column> <text>`, `/table search <text>`, `/table columns <name,...>`, and `/table reset` change only interactive view state.

`/chart <bar|hbar|line|pie> <label-column> <value-column>` builds a chart from the current filtered grid. Values must be finite numbers. A chart owns one or more named series with non-blank labels and equal category counts. Terminal rendering uses the full result width and provides a title, labels, values, axes where meaningful, and a legend for multiple series. Line charts use Braille markers. Bar, horizontal-bar, line, and proportional distribution views use distinct shapes as well as semantic series colors, so content remains understandable without color. Narrow terminals degrade to labelled textual values rather than truncating into an unreadable plot. The chart model is independent of Ratatui and future image exporters.

Table and chart results occupy the primary full-width work surface above the persistent lineage, timeline, composer, and status regions. Tables show styled headers, alternating rows, a visibly selected row, row/column counts, a visible empty state, focus-colored borders, and contextual navigation/action hints. A deterministic demo table initializes the same `DataGrid` used by loaded CSV files, so sort/filter/search/chart operations behave consistently; refresh remains unavailable until a source file is loaded. The application uses semantic theme roles (`primary`, `secondary`, `success`, `warning`, `danger`, `muted`, `text`, `focus`) rather than feature-specific hard-coded colors. Series colors come from a stable palette and repeat only after the palette is exhausted.

`/shell` creates one persistent pseudo-terminal using the platform default shell in the workspace directory. The bottom composer remains the input surface and bare input is written to that session. Output streams into the upper investigation timeline. Ctrl-C interrupts the foreground operation; `/exit` or Ctrl-D terminates the shell and restores LoreMesh command mode; `/quit` exits the application. Terminal resize is forwarded to the PTY, scrollback is bounded to 256 KiB, and Page Up/Down plus Home/End remain available. Shell commands are not added to LoreMesh history. The shell is never started implicitly.

The compatibility commands `/shell status`, `/shell enable`, `/shell disable`, and `/shell run <command>` remain recognized for bounded one-shot execution. One-shot commands run in the workspace directory with a ten-second deadline and 64 KiB per-stream capture limit.

## Domain model

`DataGrid` owns immutable headers/rows plus query, filters, sort, visible columns, and selection. `ChartModel` owns chart kind, title, categories, and named finite-value series. `ViewContent` may carry a structured chart for terminal rendering; it never stores Ratatui widgets or colors. `LoadedTable` adds a workspace-relative source path. These are report/presentation models, not canonical knowledge entities. Local process requests, PTY session handles, and results are application-boundary types.

## Interfaces

Pure grid transformations return validation errors. CSV decoding and encoding accept readers/writers at the application boundary. Chart construction consumes labelled value pairs or explicit named series. A renderer accepts `ChartModel`, terminal area, and semantic theme; it does not perform I/O or mutate chart data. The application shell session accepts input, interruption, resize, output polling, and termination operations. The presentation boundary exchanges typed input-mode transitions and content-safe responses. The legacy one-shot runner accepts command text, working directory, deadline, and output limit.

## Invariants

Rows remain rectangular; headers are non-blank and unique; at least one column remains visible; sort is stable; filters do not mutate source rows; saved CSV matches the current projection; chart titles, series names, and category labels are non-blank; chart values are finite; every series matches the category count; color is never the only carrier of meaning; paths are workspace-relative and outside `.loremesh`; existing files are never silently replaced; interactive execution cannot occur until the user enters `/shell`; only one PTY session exists per TUI; returning to LoreMesh terminates it.

## Failure modes

Missing or malformed CSV, duplicate headers, invalid UTF-8, unknown column, empty visible-column set, non-numeric chart data, unsafe path, changed/deleted refresh source, existing destination, unavailable platform shell, PTY creation failure, unexpected child exit, one-shot timeout, non-zero exit, and truncated output are reported without corrupting terminal state or discarding the last valid grid.

## Security and privacy implications

CSV and command output are untrusted input. Terminal control characters must be neutralized before display. HTML escaping remains mandatory for exports. CSV formula injection is neutralized when producing files intended for spreadsheet use. Shell entry is explicit, temporary, and never implied by loading a workspace. The child has the user's OS permissions and may access files, credentials, or networks. Commands and outputs are excluded from ordinary logs and reports. Working-directory selection is not described as sandboxing.

## Observability requirements

The composer visibly shows the active input mode; status content shows grid row counts, source staleness, chart kind, truncation, timeout, and exit category. Diagnostics may record operation names, durations, row counts, and exit categories but not cell content, commands, or process output.

## Acceptance criteria

Deterministic tests cover composed sorting/filtering/search, column visibility, CSV round trips, safe load/save/refresh, non-numeric and mismatched-series chart rejection, every terminal chart kind, multi-series legends, semantic colors, full-width result layout, narrow-terminal fallback, table empty states, explicit shell entry, input routing, return-to-LoreMesh transitions, PTY output polling, timeout, output truncation, non-zero exits, and control-sequence neutralization. All tests remain offline and use temporary directories.

## Test strategy

Use unit and property tests for grid invariants and CSV round trips, Ratatui test-backend tests for grid/chart rendering, application tests with deterministic fixture CSV files, and runner contract tests using the current test executable rather than platform network tools. Unix CI uses a portable echo marker to verify PTY input and streamed output. Windows CI verifies ConPTY creation, child liveness, input, resize, interrupt, and shutdown because GitHub-hosted service sessions may keep ConPTY children alive without exposing their output stream; maintainers must smoke-test visible interactive output on Windows before a release. Do not snapshot platform-specific prompts or arbitrary shell output.

## Deferred decisions

Cell editing, formulas, XLSX, typed/date columns, aggregation, scatter plots, chart export, runtime themes, user-defined palettes, large-file virtualization, async refresh, direct-program mode, environment allowlists, platform sandboxing, full cursor-addressed terminal emulation, persistent shell transcript export, explicit shell confirmation beyond `/shell`, and promoting reviewed command output into evidence.
