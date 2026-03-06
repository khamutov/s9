---
name: write-design-doc
description: Write technical design documents (design docs, DDs, RFCs, ADRs, tech specs). Use this skill whenever the user needs to make a non-trivial technical decision, evaluate architectural trade-offs, propose a system change, or record why a particular approach was chosen. Trigger on phrases like "design doc", "RFC", "tech spec", "architecture decision", "should we use X or Y", "evaluate options for", "write up the design for", or any request to document a technical decision with alternatives and trade-offs. Also trigger when the user describes a problem with multiple viable solutions and needs structured reasoning to pick one.
---

# Writing Design Documents

A design document records the *why* behind non-trivial technical decisions. Its purpose is not to describe implementation — it's to show that the problem was understood, alternatives were explored, and the chosen path makes sense given the constraints.

The practice originates from Google's engineering culture, where design docs serve as the primary tool for achieving consensus, surfacing cross-cutting concerns early, and building organizational memory around design decisions.

## When to write a design doc

Write a DD when the decision has consequences that will be expensive to reverse — architectural choices, technology selection, data model changes, cross-team interfaces, or migration strategies. The key signal is ambiguity: if the right solution isn't obvious, or if reasonable engineers would disagree, a DD adds value.

## When NOT to write one

Skip the DD if the solution is obvious and uncontroversial. A telltale sign you don't need one: if the document would just say "this is how we'll implement it" without discussing trade-offs or alternatives, go write the code instead. Agile methodology is not an excuse to skip thinking about known hard problems, but a DD shouldn't be busywork either.

## Document structure

Write in Markdown. Use this structure, but adapt it — not every section is mandatory for every doc. A focused 2-3 page mini-DD for a scoped decision is perfectly valid.

```markdown
# [Title]: [Brief description]

**Author(s):** [names]
**Status:** Draft | In Review | Approved | Superseded by [link]
**Date:** [created date]
**Reviewers:** [names]

## Context and Scope

## Problem Statement

## Goals and Non-goals

## Options Considered

### Option 1: [Name] `[suggested]`

### Option 2: [Name]

### Option 3: [Name] `[rejected]`

## Evaluation

## Security Considerations

## Backward Compatibility and Rollback

## Observability and Monitoring

## Open Questions
```

## Writing guidance for each section

### Context and Scope

Set the stage briefly. What system are we in? What exists today? What changed to make this decision necessary? Assume some reader familiarity — link to background material rather than repeating it. Stick to objective facts. A system-context diagram showing the target system within its broader environment is often helpful here.

### Problem Statement

Articulate what problem is being solved and why it matters now. Be specific and measurable when possible. "Improve latency" is weak. "Reduce P95 latency on profile view from 2.1s to under 1s" is strong. Tie the problem to team goals or business outcomes when relevant.

### Goals and Non-goals

Goals are what the design must achieve. Non-goals are things that *could reasonably be goals but are explicitly excluded*. Non-goals are not negated goals ("the system shouldn't crash" is not a useful non-goal). A good non-goal example: "ACID compliance" for a system where eventual consistency is acceptable — it communicates a deliberate scope boundary. Non-goals prevent scope creep and help reviewers understand what trade-offs are intentional.

### Options Considered

This is the heart of the document. For each option:

- Describe the approach at a level sufficient for the reader to evaluate its trade-offs. Resist the urge to include full API contracts, database schemas, or class hierarchies — these become noise quickly and belong in implementation. Include partial technical details only when they are the crux of the decision (e.g., a schema design that drives the entire option's viability).
- List pros and cons honestly.
- Always include "Do nothing / Status quo" as a baseline option when applicable. Sometimes the best decision is to not build anything, and this baseline helps calibrate the value of alternatives.

Mark options:

- **`[suggested]`** — the recommended option. Provide clear justification for why it best satisfies the goals given the constraints.
- **`[rejected]`** — a clearly unacceptable option. Explain why it was ruled out (e.g., doesn't meet a hard requirement, introduces unacceptable risk). Documenting rejected options prevents future engineers from re-exploring dead ends.
- Options with neither marker are viable competitors that lost on trade-offs. Their presence shows the decision was thoughtfully considered.

### Evaluation

When comparing multiple options against several factors, a comparison table helps. Evaluate options against functional requirements, non-functional requirements (performance, scalability, reliability), and other important factors (team expertise, operational cost, time to implement, migration complexity). Example:

```markdown
| Factor              | Option 1       | Option 2       | Option 3       |
|---------------------|----------------|----------------|----------------|
| Latency             | Good (<100ms)  | Acceptable     | Poor           |
| Migration effort    | 2 weeks        | 1 week         | 3 weeks        |
| Operational cost    | Low            | Medium         | Low            |
| Team familiarity    | High           | Low            | Medium         |
```

Not every DD needs a table — use one when the comparison involves 3+ options and multiple factors. For simpler decisions, inline prose in the options section is sufficient.

### Security Considerations

Assess the design for potential security implications: new attack surfaces, authentication/authorization changes, data exposure, input validation needs, secrets management. Even "no new security concerns" is worth stating explicitly, as it signals the author considered the topic.

### Backward Compatibility and Rollback

Address how the change interacts with existing systems during rollout. Long-term backward compatibility support is usually not a goal, but short-term coexistence is often critical for safe deployments. Cover: Can we roll back if something goes wrong? Is a phased rollout possible (e.g., write to both old and new, then cut over reads)? What happens to in-flight requests during migration?

### Observability and Monitoring

How will you know the new system is working correctly in production? What metrics, alerts, or dashboards are needed? This section can be brief but should exist — it demonstrates the author is thinking about the full lifecycle, not just the happy path.

### Open Questions

List unresolved questions that need input from reviewers or further investigation. This is valuable because it tells the reader where their feedback is most needed and prevents the doc from pretending to have answers it doesn't.

## Tone and style

- Write for a busy reader. The sweet spot for a larger project is around 10-20 pages. Smaller scoped decisions can and should be 1-3 pages.
- Lead with the decision and rationale. A reader should understand what was decided and why within the first few sections.
- Be direct. Hedge less. "We should use X because Y" is better than "It might be worth considering X given Y."
- Use diagrams when they clarify system relationships or data flow. A system-context diagram is frequently worth including.

## Lifecycle

Design docs are living documents during the design phase, but they become historical records after implementation. If the system hasn't shipped yet, update the doc when the design changes. After launch, it's more practical to write amendments (new docs that reference the original) rather than rewriting. Link amendments from the original doc — future readers doing "design doc archaeology" will thank you.
