"""TASKS.md parser — extracts structured task data from the markdown file."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Task:
    """A single task parsed from TASKS.md."""

    id: str
    phase: int
    status: str  # " " = pending, "~" = in-progress, "x" = completed
    description: str
    blockers: list[str] = field(default_factory=list)


# Matches lines like: - [x] **3.6** Ticket API endpoints + OpenAPI [blocked by: 0.4, 0.17, 2.14, 3.3]
_TASK_RE = re.compile(
    r"^-\s+\[(?P<status>[ ~x])\]\s+"
    r"\*\*(?P<id>\d+\.\d+)\*\*\s+"
    r"(?P<desc>.+?)$"
)

_BLOCKER_RE = re.compile(r"\[blocked by:\s*(?P<blockers>[^\]]+)\]")

_PHASE_HEADING_RE = re.compile(r"^##\s+Phase\s+(?P<phase>\d+):")


def parse_tasks(path: Path) -> list[Task]:
    """Parse TASKS.md into a list of Task objects."""
    tasks: list[Task] = []
    current_phase = 0

    for line in path.read_text().splitlines():
        phase_match = _PHASE_HEADING_RE.match(line)
        if phase_match:
            current_phase = int(phase_match.group("phase"))
            continue

        task_match = _TASK_RE.match(line.strip())
        if not task_match:
            continue

        desc = task_match.group("desc")
        blockers: list[str] = []
        blocker_match = _BLOCKER_RE.search(desc)
        if blocker_match:
            raw = blocker_match.group("blockers")
            blockers = [b.strip() for b in raw.split(",")]
            desc = desc[: blocker_match.start()].strip().rstrip("→").strip()

        tasks.append(Task(
            id=task_match.group("id"),
            phase=current_phase,
            status=task_match.group("status"),
            description=desc,
            blockers=blockers,
        ))

    return tasks


def _blocker_resolved(blocker: str, tasks: list[Task]) -> bool:
    """Check whether a single blocker string is resolved."""
    # Phase-level blocker (e.g. "Phase 3")
    phase_match = re.match(r"Phase\s+(\d+)", blocker)
    if phase_match:
        phase_num = int(phase_match.group(1))
        return all(t.status == "x" for t in tasks if t.phase == phase_num)

    # Task-level blocker (e.g. "3.6")
    for t in tasks:
        if t.id == blocker:
            return t.status == "x"

    # Unknown blocker — treat as unresolved
    return False


def find_next_task(tasks: list[Task], target_phase: int | None = None) -> Task | None:
    """Find the next actionable task.

    Prefers resuming an in-progress task, then picks the first pending task
    whose blockers are all resolved. If target_phase is set, only considers
    tasks in that phase.
    """
    candidates = tasks
    if target_phase is not None:
        candidates = [t for t in tasks if t.phase == target_phase]

    # Resume in-progress tasks first
    for t in candidates:
        if t.status == "~":
            return t

    # Find first pending task with resolved blockers
    for t in candidates:
        if t.status != " ":
            continue
        if all(_blocker_resolved(b, tasks) for b in t.blockers):
            return t

    return None


def check_completion(tasks: list[Task], target_phase: int | None = None) -> bool:
    """Check whether all relevant tasks are completed."""
    candidates = tasks
    if target_phase is not None:
        candidates = [t for t in tasks if t.phase == target_phase]
    return all(t.status == "x" for t in candidates)
