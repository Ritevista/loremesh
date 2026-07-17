# Data workbench

## Status

Proposed for the next vertical slice.

## Problem

Investigations produce tables and numeric series that need interactive inspection, local CSV interchange, charts, and occasional processing with installed command-line tools. The current TUI renders static tables and has no explicit local-execution trust boundary.

## Goals

Provide a reusable data grid, renderer-neutral chart models, deterministic terminal charts, safe local CSV load/save, refresh from the same file, and explicit bounded local shell execution.

## Non-goals

Excel formulas, cell editing, XLSX compatibility, arbitrary SQL, background schedulers, remote data sources, automatic command execution, command-output parsing as evidence, graphical PNG/SVG chart rendering, or a portable operating-system sandbox.

## User scenarios

A user loads a workspace-relative CSV, searches all visible cells, filters and sorts by named columns, hides irrelevant columns, refreshes after an external tool updates the file, and saves the current result to another CSV. The user renders a numeric column as bar, horizontal-bar, line, or pie data. After explicitly enabling local tools for the session, the user runs a visible shell pipeline and reviews bounded untrusted output.

## Functional requirements

The grid supports row selection, vertical and horizontal navigation, stable ascending/descending sort, case-insensitive global search, per-column contains filters, column visibility, reset, and a visible count of matching and total rows. Operations compose deterministically and preserve the immutable loaded rows. Unknown columns and non-rectangular input are rejected actionably.

`/table load <path>` loads UTF-8 CSV inside the workspace; `/table refresh` reloads its original path; `/table save <path>` writes the current visible rows and columns without overwriting; `/table sort <column> <asc|desc>`, `/table filter <column> <text>`, `/table search <text>`, `/table columns <name,...>`, and `/table reset` change only interactive view state.

`/chart <bar|hbar|line|pie> <label-column> <value-column>` builds a chart from the current filtered grid. Values must be finite numbers. Terminal rendering uses Unicode cells and readable labels; it remains useful without color. The chart model is independent of Ratatui and future image exporters.

`/shell status`, `/shell enable`, `/shell disable`, and `/shell run <command>` are recognized. Execution is disabled on every startup. Enablement is session-local and displays a warning that the command has the user's operating-system permissions and may access files or networks. Commands run in the workspace directory, have a default ten-second deadline, capture at most 64 KiB from each output stream while draining excess, report exit status, and never run during tests. Output is marked untrusted and is not persisted automatically.

## Domain model

`DataGrid` owns immutable headers/rows plus query, filters, sort, visible columns, and selection. `ChartModel` owns chart kind, title, and finite labelled values. `LoadedTable` adds a workspace-relative source path. These are report/presentation models, not canonical knowledge entities. Local process requests and results are application-boundary types.

## Interfaces

Pure grid transformations return validation errors. CSV decoding and encoding accept readers/writers at the application boundary. Chart construction consumes the grid's visible projection. `LocalToolRunner::run` accepts command text, working directory, deadline, and output limit and returns exit metadata plus bounded stdout/stderr.

## Invariants

Rows remain rectangular; headers are non-blank and unique; at least one column remains visible; sort is stable; filters do not mutate source rows; saved CSV matches the current projection; chart values are finite; paths are workspace-relative and outside `.loremesh`; existing files are never silently replaced; execution cannot occur before session enablement.

## Failure modes

Missing or malformed CSV, duplicate headers, invalid UTF-8, unknown column, empty visible-column set, non-numeric chart data, unsafe path, changed/deleted refresh source, existing destination, unavailable platform shell, spawn failure, timeout, non-zero exit, and truncated output are reported without terminating the TUI or discarding the last valid grid.

## Security and privacy implications

CSV and command output are untrusted input. Terminal control characters must be neutralized before display. HTML escaping remains mandatory for exports. CSV formula injection is neutralized when producing files intended for spreadsheet use. Shell enablement is explicit, temporary, and never implied by loading a workspace. Commands and outputs are excluded from ordinary logs and reports. Working-directory restriction is not described as sandboxing.

## Observability requirements

The status row shows grid row counts, source staleness, chart kind, shell enabled/disabled state, truncation, timeout, and exit category. Diagnostics may record operation names, durations, row counts, and exit categories but not cell content, commands, or process output.

## Acceptance criteria

Deterministic tests cover composed sorting/filtering/search, column visibility, CSV round trips, safe load/save/refresh, non-numeric chart rejection, each terminal chart kind, shell disabled-by-default behavior, timeout, output truncation, non-zero exits, and control-character neutralization. All tests remain offline and use temporary directories.

## Test strategy

Use unit and property tests for grid invariants and CSV round trips, Ratatui test-backend tests for grid/chart rendering, application tests with deterministic fixture CSV files, and runner contract tests using the current test executable rather than platform network tools. Do not snapshot arbitrary shell output.

## Deferred decisions

Cell editing, formulas, XLSX, typed/date columns, aggregation, multiple series, scatter plots, chart export, large-file virtualization, async refresh, direct-program mode, environment allowlists, platform sandboxing, execution confirmation policy beyond session enablement, and promoting reviewed command output into evidence.
