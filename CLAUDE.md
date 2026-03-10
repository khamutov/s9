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

# E2E testing

Playwright is set up in `frontend/e2e/`. Config: `frontend/playwright.config.ts`.

- Run: `cd frontend && npm run e2e` or `task frontend:e2e`
- Debug: `npm run e2e:ui` for interactive UI mode
- Add E2E tests for each user-facing feature that involves navigation, form submission, or data display — not for purely visual/styling work
- Place tests in `frontend/e2e/<feature>.spec.ts` (e.g., `ticket-list.spec.ts`)
- Use role-based and text-based locators (`getByRole`, `getByText`) over CSS selectors
- Tests run against the Vite dev server (auto-started by Playwright config)

# Task tracking

Active tasks are tracked in `TASKS.md` at the repository root.

Workflow:
1. Check TASKS.md for the next available task (unchecked `[ ]`, all blockers resolved `[x]`).
2. Mark it in-progress: `[ ]` → `[~]`.
3. Do the work. Deliverable path is noted in the task.
4. Mark it completed: `[~]` → `[x]`.

Status legend: `[ ]` pending, `[~]` in progress, `[x]` completed.

# UI Design System — "Warm Midnight"

All prototypes and frontend pages follow the S9 "Warm Midnight" design language. Reference implementation: `prototypes/01-ticket-list.html`.

## Typography

Use Google Fonts. Never use Inter, Roboto, Arial, or system font stacks as primary fonts.

- **Display/headings**: Bricolage Grotesque (weight 700–800, tight letter-spacing -0.04em)
- **Body text**: Instrument Sans (weight 400–600)
- **Monospace/data**: Martian Mono (IDs, timestamps, code, filter inputs, badges)

```html
<link href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,400;12..96,600;12..96,700;12..96,800&family=Instrument+Sans:wght@400;500;600;700&family=Martian+Mono:wght@400;600&display=swap" rel="stylesheet">
```

## Color Palette

Warm dark theme — all backgrounds use warm undertones, never blue-blacks.

| Token | Value | Usage |
|---|---|---|
| `--bg-root` | `#0e0d0b` | Page background |
| `--bg-surface` | `#171613` | Cards, table bg, inputs |
| `--bg-raised` | `#1e1d19` | Elevated surfaces, focus states |
| `--bg-hover` | `#242320` | Row/item hover |
| `--accent` | `#e8b43a` | Primary accent (golden) |
| `--accent-hover` | `#d4a230` | Accent hover state |
| `--text-primary` | `#ede9dd` | Main text (warm cream) |
| `--text-label` | `#a89d90` | Small readable text — mono labels, table headers, timestamps, counts (7.3:1 on root) |
| `--text-secondary` | `#8c8579` | Supporting body text at ≥13px (5.3:1 on root) |
| `--text-tertiary` | `#5c564d` | Decorative hints only — placeholders, optional tags (2.7:1, not for readable text) |
| `--text-ghost` | `#3d3830` | Dot-grid texture, borders, scrollbar (not for any text) |
| `--border-primary` | `rgba(255,245,220,0.08)` | Borders |
| `--border-subtle` | `rgba(255,245,220,0.04)` | Row separators |

Status colors (tuned for dark bg): new=`#8c8579`, progress=`#7cb8f7`, verify=`#e8b43a`, done=`#5eca7e`.
Priority colors: P0=`#f87171`, P1=`#fb923c`, P2=`#e8b43a`, P3=`#7cb8f7`, P4=`#5c564d`, P5=`#3d3830`.

## Motion

- **Page load**: stagger child elements with `animation-delay` increments of ~40ms using `fade-down` (translateY(-12px) + opacity) or `row-reveal` (translateY(6px) + opacity)
- **Easing**: `cubic-bezier(0.16, 1, 0.3, 1)` for all transitions
- **Hover transitions**: 150ms ease-out
- **Pulsing dots**: use on "In Progress" status badges and P0 urgency indicators
- Keep animations subtle — one orchestrated load sequence per page, avoid scattered micro-interactions

## Background & Atmosphere

- Dot-grid texture via `body::before` using `radial-gradient` (0.5px dots at 24px spacing, opacity 0.3)
- Ambient accent glow via `radial-gradient(circle, rgba(232,180,58,0.03), transparent)` positioned top-right
- Sidebar uses vertical gradient (`#131210` → `#0a0908`)

## Component Patterns

- **Sidebar**: dark gradient bg, section labels in mono uppercase (9px, 0.12em tracking), active item has 3px golden left bar + golden text, badge counts in mono with pill background
- **Tables**: wrapped in `.table-wrap` (border + radius), no `table-layout: fixed`, headers in mono uppercase (10px, 0.1em tracking), row hover highlights ID in accent color, selected row gets 3px golden left bar
- **Status badges**: rounded rect with colored dot `::before`, "In Progress" dot pulses
- **Priority**: signal bar icons (4 ascending bars, filled count indicates severity) + mono label
- **Buttons**: primary = golden bg with dark text, inset highlight, shadow glow, hover lifts 1px
- **Inputs**: dark surface bg, golden focus ring (`box-shadow: 0 0 0 3px`) + glow, mono font for filter/search
- **User avatars**: 22px circles with 2-letter initials, distinct hue per person (indigo=#a78bfa, teal=#2dd4bf, amber=#e8b43a, rose=#fb7185)

## Design Principles

- Avoid generic "AI slop" aesthetics: no purple gradients on white, no evenly-distributed timid palettes
- Commit to the warm dark theme — dominant golden accent with sharp status-color accents
- Use mono font for anything data-oriented (IDs, counts, timestamps, labels, filter inputs)
- Keep information dense but scannable — no excessive whitespace
- Every interactive element needs a visible hover/focus state
- **Contrast**: All readable text must meet WCAG AAA (7:1) against its background. Small mono text (≤11px labels, table headers, timestamps, counts) uses `--text-label` (#a89d90, 7.3:1 on root). Body-size supporting text (≥13px) uses `--text-secondary` (#8c8579, 5.3:1). `--text-tertiary` and `--text-ghost` are never used for text that users need to read — only for decorative hints, placeholders, borders, and textures. Always verify contrast ratios when introducing new colors or placing text on non-standard backgrounds.
