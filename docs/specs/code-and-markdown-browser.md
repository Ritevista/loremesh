# Code and rich Markdown browser

## Status

Accepted. The initial bounded file listing/open/search and semantic Markdown/diagram-source subset is implemented; recursive tree interaction and external renderers remain deferred.

## Problem

Investigations need to inspect source files and Markdown without leaving the terminal. Plain text loses document structure, while Mermaid and D2 fences currently remain export source rather than useful interactive diagrams. Unrestricted filesystem browsing or renderer subprocesses could expose private files, follow unsafe links, or execute untrusted content.

## Goals

Provide a reusable, read-only code browser; structured Markdown presentation; safe workspace search and navigation; evidence-friendly line references; and useful Mermaid/D2 views that work offline with a source fallback.

## Non-goals

Editing files, an IDE or language server, compilation, arbitrary repository execution, full Git history, full Mermaid/D2 language reimplementation, remote asset fetching, embedded browser engines, automatic renderer installation, or treating viewed files as imported authoritative artifacts.

## User scenarios

A user opens a workspace tree, filters filenames, opens a UTF-8 source file with line numbers, searches within it, follows a relative Markdown link, and creates an artifact reference for selected lines. The user opens Markdown with headings, lists, tables, code fences, and links rendered semantically. Mermaid or D2 fences display a terminal graph when supported and always allow switching to the original source.

## Functional requirements

`/browse [path]` opens a tree rooted inside the workspace. The tree supports expand/collapse, filename filtering, refresh, keyboard selection, and ignores `.git`, `.loremesh`, build outputs, and configured patterns by default. `/open <path>[:line]` opens a bounded text file. The viewer provides line numbers, horizontal/vertical scrolling, visible whitespace toggle, case-sensitive or insensitive search, next/previous match, and copy/save-reference actions. Binary and oversized files show metadata and a useful refusal rather than being decoded blindly.

Paths are resolved canonically beneath the workspace root. Symlinks that resolve outside it are not followed. Browsing is read-only and does not import content automatically. A reference created from selected lines records the actual imported artifact and immutable snapshot; a merely browsed mutable file cannot masquerade as evidence.

Markdown presentation supports headings, paragraphs, emphasis, lists, block quotes, thematic breaks, code fences, tables, local links, and images as labelled references. Relative links are resolved beneath the workspace and require an explicit open action. HTTP links are labelled external and never fetched automatically. Raw HTML is displayed as inert text in the terminal.

Fenced `mermaid` and `d2` blocks retain exact source and expose Source and Diagram modes. A small documented graph subset is projected into the renderer-neutral trace/graph view for deterministic terminal display. Unsupported syntax produces a diagnostic and falls back to highlighted source. An optional, explicitly configured local subprocess renderer may later produce sanitized SVG or PNG exports with a timeout, bounded output, no network promise, and version metadata; its absence never degrades ordinary Markdown reading.

Commands include `/browse`, `/open`, `/find`, `/search-next`, `/search-prev`, `/markdown source|rendered`, and `/diagram source|rendered`. The direct `f` shortcut is the normal way to search the focused document: it accepts plain text in a labelled Find composer and leaves the document visible while reporting matching locations. `/find <text>` remains an automation/accessibility equivalent and works for both mutable workspace files and canonical artifacts opened from corpus results. `/search <text>` is reserved for canonical knowledge-corpus search so users never accidentally search the wrong scope. `b` returns from an opened canonical artifact to its prior result table. Names may be consolidated when command completion is introduced, but observable behavior and safe path rules remain stable.

## Domain model

`FileTreeEntry`, `CodeDocument`, `TextSelection`, `MarkdownDocument`, `MarkdownBlock`, `DiagramSource`, and `DiagramView` are presentation/application models. Evidence references continue to use canonical core artifact and snapshot identifiers. Browser state and mutable file paths are never domain evidence by themselves.

## Interfaces

A workspace file-reader port lists safe entries, reads bounded bytes, canonicalizes paths, and returns file metadata without exposing storage internals. Pure parsers create Markdown blocks and the supported diagram graph. Terminal widgets consume these models. Optional renderer adapters consume diagram source and return typed, size-bounded render results plus renderer identity.

## Invariants

Every opened path remains below the canonical workspace root; excluded internal directories stay hidden unless a future explicit diagnostic mode permits them; browsing never mutates or imports; line numbers map to original bytes; selections use valid UTF-8 boundaries; Markdown links never trigger I/O automatically; diagram source is always recoverable; renderer failure never removes the source fallback.

## Failure modes

Missing files, permission denial, symlink escape, path traversal, invalid UTF-8, binary input, oversized input, refresh races, invalid line selection, unsupported Markdown extension, malformed diagram, absent renderer, renderer timeout, excessive output, and unsafe generated SVG are reported inside the active view without terminating the TUI.

## Security and privacy implications

Files and markup are untrusted. Terminal control characters are neutralized. Raw HTML and diagram labels cannot execute. External links and image URLs are never fetched implicitly. Errors and logs avoid file bodies and redact absolute path prefixes. Optional renderers use the same explicit subprocess trust boundary as local tools, but configuring a renderer does not enable the general shell. Generated SVG is not embedded until a sanitizer rejects scripts, event handlers, foreign objects, and external references.

## Observability requirements

The status row may show relative path, language hint, line/byte counts, search match position, truncation, Markdown mode, diagram mode, and renderer availability. Diagnostics record operation category, relative-path hash, byte count, duration, and outcome—not content, search terms, selections, or absolute paths.

## Acceptance criteria

Tests cover path traversal and symlink escape rejection, exclusions, deterministic tree ordering, bounded reads, line mapping, search navigation, control-character neutralization, semantic Markdown blocks, inert raw HTML and external links, Mermaid/D2 subset projection, unsupported-syntax fallback, absent-renderer behavior, and evidence creation only after import. Tests use generic temporary fixtures and no network.

## Test strategy

Use unit and property tests for path and selection invariants, parser fixtures for Markdown and diagram subsets, temporary-directory contract tests for file browsing, Ratatui test-backend tests for code/Markdown/diagram modes, and regression fixtures for malformed markup and terminal escape sequences.

## Deferred decisions

Syntax-highlighting dependency, Tree-sitter, language-server integration, Git-aware navigation, blame/diff views, very-large-file virtualization, mouse selection, clipboard integration, full Mermaid/D2 coverage, renderer installation discovery, SVG sanitizer choice, terminal image protocols, and promotion of browsed selections into imported evidence.
