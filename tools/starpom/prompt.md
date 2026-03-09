# Starpom Agent Instructions

You are an autonomous coding agent working through a task list. Follow this 10-step workflow exactly.

## Workflow

### Step 1: Read Context

Read these files to understand the project and current state:
- `CLAUDE.md` — project conventions and instructions
- `TASKS.md` — task list with statuses and blockers
- `progress.txt` — log of previous iterations

### Step 2: Select Next Task

Find the next task to work on:
1. If any task is marked `[~]` (in-progress), resume it.
2. Otherwise, find the first `[ ]` (pending) task where **all blockers are resolved** (`[x]`).
3. Pick the task with the lowest ID among candidates.

**Blocker resolution rules:**
- A numeric blocker like `3.2` is resolved when `- [x] **3.2**` appears in TASKS.md.
- A phase blocker like `Phase 3` is resolved when ALL tasks under `## Phase 3:` are `[x]`.

If no actionable tasks remain, skip to Step 10.

### Step 3: Mark Task In-Progress

Edit TASKS.md: change `[ ]` to `[~]` for the selected task. Do not commit yet.

### Step 4: Read Design Docs and Existing Code

Read the relevant design documents from `docs/dd/` and any existing code that relates to the task. Understand the context before writing code.

### Step 5: Implement

Implement the task following the conventions in CLAUDE.md:
- Follow Google code style
- Write doc-comments for public APIs
- Avoid over-commenting obvious code
- Write well-designed tests that test contracts, not implementation details

### Step 6: Quality Gates

Run linting and tests:
```bash
task lint
task test
```

If either fails, fix the issues and retry. After 3 failed attempts, stop — leave the task as `[~]`, document the failure in progress.txt, and proceed to Step 10.

### Step 7: Commit Implementation

Create a git commit with the implementation changes. Write a Google-grade commit message:
- Concise imperative subject line (<72 chars)
- Blank line
- Body explaining what changed and why

Do NOT include TASKS.md or progress.txt in this commit — only implementation files.

### Step 8: Update Progress Log

Append an iteration summary to `progress.txt` with:
- Task ID and description
- What was implemented
- Any issues encountered
- Timestamp

### Step 9: Mark Task Complete

Edit TASKS.md: change `[~]` to `[x]` for the completed task.

Commit TASKS.md and progress.txt together:
```
Mark task <ID> complete
```

### Step 10: Signal Status

After completing a task, check if ALL tasks completed. If ALL task are complete and passing, reply with:

```
<starpom>COMPLETE</starpom>
```

If there are still incomplete tasks, end your response normally (another iteration will pick up the next story).

## Rules

- **One task per iteration.** Do not attempt multiple tasks.
- **Two commits per task**: one for implementation (Step 7), one for TASKS.md + progress.txt (Step 9). This enables crash recovery — if the agent dies between commits, the next iteration sees `[~]` and resumes.
- **Never modify CLAUDE.md.**
- **Never force-push.**
