"""Pure planning and execution logic for organizing a folder.

Planning (``build_plan``) is separated from execution (``execute_plan``) so
that dry runs and tests can inspect exactly what would happen without any
filesystem changes.
"""

from __future__ import annotations

import shutil
from dataclasses import dataclass, field
from pathlib import Path

NO_EXTENSION_FOLDER = "NO_EXTENSION_Files"


@dataclass
class PlannedMove:
    source: str
    dest_folder: str
    final_name: str
    renamed: bool


@dataclass
class SkippedItem:
    name: str
    reason: str


@dataclass
class Plan:
    folder: Path
    moves: list[PlannedMove] = field(default_factory=list)
    new_folders: list[str] = field(default_factory=list)
    skipped: list[SkippedItem] = field(default_factory=list)


@dataclass
class MoveError:
    source: str
    message: str


@dataclass
class RunResult:
    plan: Plan
    moved: list[PlannedMove] = field(default_factory=list)
    errors: list[MoveError] = field(default_factory=list)


def get_extension(name: str) -> str | None:
    """Return the lowercased extension of ``name``, or None.

    The extension is the substring after the last dot. A dot at position 0
    (dotfiles like ``.gitignore``) or a trailing dot does not count.
    """
    dot = name.rfind(".")
    if dot <= 0:
        return None
    ext = name[dot + 1 :]
    return ext.lower() if ext else None


def folder_name_for(ext: str | None) -> str:
    """Return the type-folder name for an extension (``txt`` -> ``TXT_Files``)."""
    if ext is None:
        return NO_EXTENSION_FOLDER
    return f"{ext.upper()}_Files"


def resolve_name(name: str, taken: set[str]) -> tuple[str, bool]:
    """Return a destination filename that avoids ``taken`` (lowercased names).

    On collision, the lowest free ``_N`` suffix is inserted before the
    extension (``report.txt`` -> ``report_1.txt``) or appended for
    extensionless names (``Makefile`` -> ``Makefile_1``).
    """
    if name.lower() not in taken:
        return name, False
    dot = name.rfind(".")
    if dot <= 0:
        stem, suffix = name, ""
    else:
        stem, suffix = name[:dot], name[dot:]
    counter = 1
    while True:
        candidate = f"{stem}_{counter}{suffix}"
        if candidate.lower() not in taken:
            return candidate, True
        counter += 1


def build_plan(folder: Path) -> Plan:
    """Scan the top level of ``folder`` and plan every move without executing it."""
    plan = Plan(folder=folder)
    taken: dict[str, set[str]] = {}
    for entry in sorted(folder.iterdir(), key=lambda p: p.name.lower()):
        if entry.is_symlink():
            plan.skipped.append(SkippedItem(entry.name, "symlink"))
            continue
        if entry.is_dir():
            plan.skipped.append(SkippedItem(entry.name, "directory"))
            continue
        if not entry.is_file():
            plan.skipped.append(SkippedItem(entry.name, "not a regular file"))
            continue
        dest_folder = folder_name_for(get_extension(entry.name))
        if dest_folder not in taken:
            dest_dir = folder / dest_folder
            if dest_dir.is_dir():
                taken[dest_folder] = {p.name.lower() for p in dest_dir.iterdir()}
            else:
                taken[dest_folder] = set()
                plan.new_folders.append(dest_folder)
        final_name, renamed = resolve_name(entry.name, taken[dest_folder])
        taken[dest_folder].add(final_name.lower())
        plan.moves.append(PlannedMove(entry.name, dest_folder, final_name, renamed))
    return plan


def execute_plan(plan: Plan) -> RunResult:
    """Execute every planned move, collecting per-file errors instead of aborting."""
    result = RunResult(plan=plan)
    for move in plan.moves:
        source = plan.folder / move.source
        dest_dir = plan.folder / move.dest_folder
        try:
            dest_dir.mkdir(exist_ok=True)
            shutil.move(str(source), str(dest_dir / move.final_name))
        except OSError as exc:
            result.errors.append(MoveError(move.source, str(exc)))
        else:
            result.moved.append(move)
    return result
