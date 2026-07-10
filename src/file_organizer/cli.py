"""Command-line interface: argument parsing, validation, and exit codes."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from file_organizer import __version__
from file_organizer.organizer import build_plan, execute_plan
from file_organizer.report import format_report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="file-organizer",
        description=(
            "Organize all files directly inside FOLDER into subfolders named"
            " after each file's extension (e.g. notes.txt -> TXT_Files/)."
        ),
    )
    parser.add_argument(
        "folder",
        help="path to the folder whose top-level files will be organized",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="preview all actions without changing the filesystem",
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    args = parser.parse_args(argv)

    target = Path(args.folder)
    if not target.exists():
        print(f"Error: path does not exist: {args.folder}", file=sys.stderr)
        return 2
    if not target.is_dir():
        print(f"Error: path is not a directory: {args.folder}", file=sys.stderr)
        return 2

    plan = build_plan(target.resolve())
    if args.dry_run:
        print(format_report(plan, None, dry_run=True))
        return 0

    result = execute_plan(plan)
    print(format_report(plan, result, dry_run=False))
    return 1 if result.errors else 0


def entry() -> None:
    sys.exit(main())
