# ADR 0009: Use a persistent PTY-backed local shell

## Status

Accepted.

## Context

The bounded one-shot runner is useful for automation, but `/shell run bash -lc ...` is not a natural interactive workflow. Users need persistent working-directory and environment state, streamed output, familiar prompts, interruption, and an explicit return to LoreMesh without closing the TUI. Implementing cross-platform pseudo-terminal support inside LoreMesh would be difficult to maintain.

## Decision

`/shell` starts the platform default program through `portable-pty` in the workspace directory. The application composition root owns the child, PTY handles, bounded in-memory scrollback, resize requests, and output reader thread. The presentation crate owns only typed mode transitions and keystrokes. Bare composer input is sent to the child; Ctrl-C interrupts it; `/exit` and Ctrl-D terminate it and return to LoreMesh; `/quit` exits the application. Dropping a session makes a best-effort child termination.

PTY output is untrusted and has common terminal control sequences removed before display. Scrollback is bounded to 256 KiB and is not logged, exported, or promoted to evidence automatically. The shell inherits the user's operating-system authority: its workspace working directory is convenience, not a sandbox. The existing explicit one-shot runner remains temporarily available for compatibility.

## Consequences

Shell state such as `cd` and exported variables persists for the TUI session, and output streams without blocking the event loop. `portable-pty` adds a focused cross-platform process dependency and transitive platform crates; updates require Unix and Windows CI. The current display is a sanitized text transcript, not a full terminal emulator, so full-screen programs and exact cursor-addressed rendering are deferred.

## Alternatives considered

Repeated platform shell invocations cannot preserve state or provide a normal prompt. Embedding a terminal emulator would significantly expand scope. Temporarily leaving the alternate screen for an external terminal would break the shared investigation timeline and future chat interaction model.
