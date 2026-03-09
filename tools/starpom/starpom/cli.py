"""CLI argument parsing and entry point."""

import argparse
import sys

from .runner import run_loop


def build_parser() -> argparse.ArgumentParser:
    """Build the argument parser."""
    parser = argparse.ArgumentParser(
        prog="starpom",
        description="Autonomous agent loop — picks tasks from TASKS.md and drives Claude to implement them.",
    )
    parser.add_argument(
        "--max-iterations",
        type=int,
        default=50,
        help="Maximum number of agent iterations (default: 50)",
    )
    parser.add_argument(
        "--phase",
        type=int,
        default=None,
        help="Scope work to a specific phase number",
    )
    parser.add_argument(
        "--clean",
        action="store_true",
        help="Reset progress.txt before starting",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the prompt that would be sent and exit",
    )
    return parser


def main() -> None:
    """Entry point."""
    parser = build_parser()
    args = parser.parse_args()
    try:
        run_loop(args)
    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(130)
