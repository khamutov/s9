"""Iteration loop and Claude subprocess orchestration."""

from __future__ import annotations

import shutil
import subprocess
import sys
import time
from argparse import Namespace
from datetime import datetime, timezone
from pathlib import Path

from .tasks import check_completion, find_next_task, parse_tasks

COMPLETION_SENTINEL = "<starpom>COMPLETE</starpom>"


def _repo_root() -> Path:
    """Resolve the git repository root."""
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True, text=True, check=True,
    )
    return Path(result.stdout.strip())


def _preflight(repo_root: Path) -> None:
    """Run pre-flight checks before starting the loop."""
    # Check claude is available
    if not shutil.which("claude"):
        print("ERROR: 'claude' not found in PATH.", file=sys.stderr)
        sys.exit(1)

    # Check we're in a git repo (already confirmed by _repo_root)

    # Warn on uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        capture_output=True, text=True, cwd=repo_root,
    )
    if result.stdout.strip():
        print("WARNING: Uncommitted changes detected. Proceeding anyway.\n")

    # Check TASKS.md exists
    tasks_path = repo_root / "TASKS.md"
    if not tasks_path.exists():
        print(f"ERROR: {tasks_path} not found.", file=sys.stderr)
        sys.exit(1)


def _build_prompt(repo_root: Path, target_phase: int | None) -> str:
    """Load prompt.md and append phase focus suffix if needed."""
    prompt_path = Path(__file__).parent.parent / "prompt.md"
    prompt = prompt_path.read_text()

    if target_phase is not None:
        prompt += f"\n\n**PHASE FOCUS**: Only work on Phase {target_phase} tasks.\n"

    return prompt


def run_iteration(prompt: str, iteration: int, repo_root: Path) -> str:
    """Run a single Claude invocation via `claude --dangerously-skip-permissions --print`.

    Streams stdout line-by-line as it arrives and returns the full output.
    """
    proc = subprocess.Popen(
        ["claude", "--dangerously-skip-permissions", "--print", "--chrome", prompt],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=repo_root,
    )

    lines: list[str] = []
    assert proc.stdout is not None  # noqa: S101 — guaranteed by PIPE
    for line in proc.stdout:
        print(line, end="", flush=True)
        lines.append(line)

    stderr = proc.stderr.read() if proc.stderr else ""
    if stderr:
        print(stderr, file=sys.stderr, flush=True)

    proc.wait()

    return "".join(lines)


def run_loop(args: Namespace) -> None:
    """Main loop: pre-flight checks, iterate, summarize."""
    repo_root = _repo_root()
    _preflight(repo_root)

    tasks_path = repo_root / "TASKS.md"
    progress_path = repo_root / "progress.txt"

    # Handle --clean
    if args.clean or not progress_path.exists():
        progress_path.write_text(
            f"# Starpom Progress Log\n\nStarted: {datetime.now(timezone.utc).isoformat()}\n\n"
        )
        print("Initialized progress.txt")

    # Parse tasks and check completion
    tasks = parse_tasks(tasks_path)
    if check_completion(tasks, args.phase):
        phase_label = f"Phase {args.phase}" if args.phase else "All"
        print(f"{phase_label} tasks already completed.")
        return

    # Show next task
    next_task = find_next_task(tasks, args.phase)
    if next_task:
        print(f"Next task: {next_task.id} — {next_task.description}")
    else:
        print("No actionable tasks found (all remaining are blocked).")
        return

    # Build prompt
    prompt = _build_prompt(repo_root, args.phase)

    # Handle --dry-run
    if args.dry_run:
        print("\n--- DRY RUN: Prompt that would be sent ---\n")
        print(prompt)
        return

    # Main loop
    completed = 0
    for i in range(1, args.max_iterations + 1):
        ts = datetime.now(timezone.utc).strftime("%H:%M:%S UTC")
        print(f"\n{'='*60}")
        print(f"  Iteration {i}/{args.max_iterations}  |  {ts}")
        print(f"{'='*60}\n")

        output = run_iteration(prompt, i, repo_root)
        completed += 1

        # Check for completion sentinel
        if COMPLETION_SENTINEL in output:
            print("\nAgent signaled COMPLETE.")
            break

        # Re-parse tasks and check
        tasks = parse_tasks(tasks_path)
        if check_completion(tasks, args.phase):
            phase_label = f"Phase {args.phase}" if args.phase else "All"
            print(f"\n{phase_label} tasks completed.")
            break

        next_task = find_next_task(tasks, args.phase)
        if next_task:
            print(f"\nNext task: {next_task.id} — {next_task.description}")
        else:
            print("\nNo more actionable tasks.")
            break

        # Brief pause between iterations
        if i < args.max_iterations:
            time.sleep(3)

    # Summary
    print(f"\n{'='*60}")
    print(f"  Starpom finished: {completed} iteration(s) completed")
    print(f"{'='*60}")
