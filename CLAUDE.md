# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Directory layout

It's a monorepository containing all components and documents. Each component/service has it's own folder, feel free to use folder hierarchy for better organization.

Top-level folders:
- docs: projects documents
- deploy: deployment scripts. For clear separation between deployment and development

## Documentation

docs/prd - PRD documents
docs/dd - Design documents (DD)

# Code conventions

- resist over-commenting obvious things or steps
- follow Google code style when applicable
- write doc-comments for each public api (everything exported from module, pub structs, methods, and etc) and non-obvious internal (so everything except self-explanatory small functions)
- write good designed tests, prefer tesing contracts, avoid testing internals/implementation-specifics except when it has a reason for that (e.g., to cover bug in implementation)
- check code with linters

# Task tracking

Active tasks are tracked in `TASKS.md` at the repository root.

Workflow:
1. Check TASKS.md for the next available task (unchecked `[ ]`, all blockers resolved `[x]`).
2. Mark it in-progress: `[ ]` → `[~]`.
3. Do the work. Deliverable path is noted in the task.
4. Mark it completed: `[~]` → `[x]`.

Status legend: `[ ]` pending, `[~]` in progress, `[x]` completed.
