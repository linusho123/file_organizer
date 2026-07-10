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
MANIFEST_NAME = ".file_organizer_manifest.json"


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
    keep_structure: bool = False
    removable_source_dirs: list[str] = field(default_factory=list)


@dataclass
class MoveError:
    source: str
    message: str


@dataclass
class RunResult:
    plan: Plan
    moved: list[PlannedMove] = field(default_factory=list)
    errors: list[MoveError] = field(default_factory=list)
    removed_source_dirs: list[str] = field(default_factory=list)


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


TYPE_FOLDER_SUFFIX = "_Files"


def is_type_folder(name: str) -> bool:
    """Return True if ``name`` looks like a folder this tool creates as a destination."""
    if name == NO_EXTENSION_FOLDER:
        return True
    if not name.endswith(TYPE_FOLDER_SUFFIX):
        return False
    prefix = name[: -len(TYPE_FOLDER_SUFFIX)]
    return bool(prefix) and prefix == prefix.upper()


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


def build_plan(folder: Path, recursive: bool = False, keep_structure: bool = False) -> Plan:
    """Scan ``folder`` and plan every move without executing anything.

    By default only direct children are considered; with ``recursive`` the
    scan descends into subfolders (except top-level type folders, which are
    destinations) and sources are recorded as forward-slash relative paths.
    With ``keep_structure`` each file's destination mirrors its source
    subpath inside the type folder, and source folders that the run would
    empty are planned for removal.
    """
    plan = Plan(folder=folder, keep_structure=keep_structure)
    files: list[str] = []
    source_dirs: list[str] = []

    def scan(directory: Path, top: bool) -> None:
        for entry in sorted(directory.iterdir(), key=lambda p: p.name.lower()):
            rel = entry.relative_to(folder).as_posix()
            if entry.name == MANIFEST_NAME:
                plan.skipped.append(SkippedItem(rel, "manifest"))
                continue
            if entry.is_symlink():
                plan.skipped.append(SkippedItem(rel, "symlink"))
                continue
            if entry.is_dir():
                if not recursive:
                    plan.skipped.append(SkippedItem(rel, "directory"))
                elif top and is_type_folder(entry.name):
                    plan.skipped.append(SkippedItem(rel, "type folder"))
                else:
                    source_dirs.append(rel)
                    scan(entry, top=False)
                continue
            if not entry.is_file():
                plan.skipped.append(SkippedItem(rel, "not a regular file"))
                continue
            files.append(rel)

    scan(folder, top=True)
    files.sort(key=str.lower)

    seen_dest: set[str] = set()
    taken: dict[tuple[str, str], set[str]] = {}
    for rel in files:
        parent, _, basename = rel.rpartition("/")
        dest_folder = folder_name_for(get_extension(basename))
        if dest_folder not in seen_dest:
            seen_dest.add(dest_folder)
            if not (folder / dest_folder).is_dir():
                plan.new_folders.append(dest_folder)
        dest_parent = parent if keep_structure else ""
        key = (dest_folder, dest_parent)
        if key not in taken:
            dest_dir = folder / dest_folder / dest_parent if dest_parent else folder / dest_folder
            if dest_dir.is_dir():
                taken[key] = {p.name.lower() for p in dest_dir.iterdir()}
            else:
                taken[key] = set()
        final_base, renamed = resolve_name(basename, taken[key])
        taken[key].add(final_base.lower())
        final_name = f"{dest_parent}/{final_base}" if dest_parent else final_base
        plan.moves.append(PlannedMove(rel, dest_folder, final_name, renamed))

    if keep_structure:
        plan.removable_source_dirs = _plan_removals(folder, source_dirs, plan.moves)
    return plan


def _plan_removals(folder: Path, source_dirs: list[str], moves: list[PlannedMove]) -> list[str]:
    """Return source dirs the run empties: >=1 moved file under them, nothing left over."""
    moved = {m.source for m in moves}
    removable: set[str] = set()
    for rel in sorted(source_dirs, key=lambda r: r.count("/"), reverse=True):
        if not any(m.startswith(f"{rel}/") for m in moved):
            continue
        for child in (folder / rel).iterdir():
            child_rel = child.relative_to(folder).as_posix()
            if child.is_file() and not child.is_symlink() and child_rel in moved:
                continue
            if child.is_dir() and not child.is_symlink() and child_rel in removable:
                continue
            break
        else:
            removable.add(rel)
    return sorted(removable, key=str.lower)


def execute_plan(plan: Plan) -> RunResult:
    """Execute every planned move, collecting per-file errors instead of aborting."""
    result = RunResult(plan=plan)
    for move in plan.moves:
        source = plan.folder / move.source
        destination = plan.folder / move.dest_folder / move.final_name
        try:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(source), str(destination))
        except OSError as exc:
            result.errors.append(MoveError(move.source, str(exc)))
        else:
            result.moved.append(move)
    for rel in sorted(plan.removable_source_dirs, key=lambda r: r.count("/"), reverse=True):
        try:
            (plan.folder / rel).rmdir()
        except OSError:
            continue
        result.removed_source_dirs.append(rel)
    return result
