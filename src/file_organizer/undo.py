"""Manifest persistence and undo of the most recent organizing run.

A real run that moves at least one file writes a JSON manifest into the
target folder. ``--undo`` reads it, moves every recorded file back to its
original top-level name, removes type folders the run created if they are
now empty, and deletes the manifest (or rewrites it with only the entries
that could not be restored, so the undo can be retried).
"""

from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from file_organizer.organizer import MANIFEST_NAME, MoveError, RunResult, resolve_name

MANIFEST_VERSION = 1


class ManifestError(Exception):
    """Raised when a manifest exists but cannot be parsed."""


@dataclass
class RecordedMove:
    source: str
    dest_folder: str
    final_name: str


@dataclass
class Manifest:
    moves: list[RecordedMove] = field(default_factory=list)
    new_folders: list[str] = field(default_factory=list)


@dataclass
class PlannedRestore:
    source: str
    dest_folder: str
    final_name: str
    restore_name: str
    renamed: bool


@dataclass
class UndoPlan:
    folder: Path
    restores: list[PlannedRestore] = field(default_factory=list)
    missing: list[RecordedMove] = field(default_factory=list)
    removable_folders: list[str] = field(default_factory=list)
    manifest: Manifest | None = None


@dataclass
class UndoResult:
    plan: UndoPlan
    restored: list[PlannedRestore] = field(default_factory=list)
    removed_folders: list[str] = field(default_factory=list)
    errors: list[MoveError] = field(default_factory=list)


def write_manifest(folder: Path, result: RunResult) -> None:
    """Persist the moves of a run; runs that moved nothing leave any manifest alone."""
    if not result.moved:
        return
    payload = {
        "version": MANIFEST_VERSION,
        "created": datetime.now(timezone.utc).isoformat(),
        "moves": [
            {"source": m.source, "dest_folder": m.dest_folder, "final_name": m.final_name}
            for m in result.moved
        ],
        "new_folders": list(result.plan.new_folders),
    }
    (folder / MANIFEST_NAME).write_text(json.dumps(payload, indent=2))


def _write_back(folder: Path, moves: list[RecordedMove], new_folders: list[str]) -> None:
    payload = {
        "version": MANIFEST_VERSION,
        "created": datetime.now(timezone.utc).isoformat(),
        "moves": [
            {"source": m.source, "dest_folder": m.dest_folder, "final_name": m.final_name}
            for m in moves
        ],
        "new_folders": new_folders,
    }
    (folder / MANIFEST_NAME).write_text(json.dumps(payload, indent=2))


def read_manifest(folder: Path) -> Manifest | None:
    """Return the folder's manifest, None if absent; raise ManifestError if unreadable."""
    path = folder / MANIFEST_NAME
    if not path.is_file():
        return None
    try:
        raw = json.loads(path.read_text())
        moves = [RecordedMove(m["source"], m["dest_folder"], m["final_name"]) for m in raw["moves"]]
        return Manifest(moves=moves, new_folders=list(raw.get("new_folders", [])))
    except (json.JSONDecodeError, KeyError, TypeError) as exc:
        raise ManifestError(str(exc)) from exc


def build_undo_plan(folder: Path, manifest: Manifest) -> UndoPlan:
    """Plan every restore and folder removal without executing anything."""
    plan = UndoPlan(folder=folder, manifest=manifest)
    taken_by_dir: dict[str, set[str]] = {}
    for move in manifest.moves:
        current = folder / move.dest_folder / move.final_name
        if not current.is_file():
            plan.missing.append(move)
            continue
        parent, _, base = move.source.rpartition("/")
        if parent not in taken_by_dir:
            parent_dir = folder / parent if parent else folder
            if parent_dir.is_dir():
                taken_by_dir[parent] = {p.name.lower() for p in parent_dir.iterdir()}
            else:
                taken_by_dir[parent] = set()
        restore_base, renamed = resolve_name(base, taken_by_dir[parent])
        taken_by_dir[parent].add(restore_base.lower())
        restore_name = f"{parent}/{restore_base}" if parent else restore_base
        plan.restores.append(
            PlannedRestore(move.source, move.dest_folder, move.final_name, restore_name, renamed)
        )
    restored_away = {(r.dest_folder.lower(), r.final_name.lower()) for r in plan.restores}
    for name in manifest.new_folders:
        folder_path = folder / name
        if not folder_path.is_dir():
            continue
        leftovers = [
            p.name
            for p in folder_path.iterdir()
            if (name.lower(), p.name.lower()) not in restored_away
        ]
        if not leftovers:
            plan.removable_folders.append(name)
    return plan


def execute_undo(plan: UndoPlan) -> UndoResult:
    """Restore files, remove now-empty created folders, and consume the manifest."""
    result = UndoResult(plan=plan)
    failed: list[RecordedMove] = []
    for move in plan.missing:
        result.errors.append(MoveError(f"{move.dest_folder}/{move.final_name}", "file not found"))
        failed.append(move)
    for restore in plan.restores:
        source = plan.folder / restore.dest_folder / restore.final_name
        destination = plan.folder / restore.restore_name
        try:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(source), str(destination))
        except OSError as exc:
            result.errors.append(MoveError(f"{restore.dest_folder}/{restore.final_name}", str(exc)))
            failed.append(RecordedMove(restore.source, restore.dest_folder, restore.final_name))
        else:
            result.restored.append(restore)
    for name in plan.removable_folders:
        try:
            (plan.folder / name).rmdir()
        except OSError:
            continue
        result.removed_folders.append(name)
    manifest_path = plan.folder / MANIFEST_NAME
    if failed:
        kept_folders = [
            f
            for f in (plan.manifest.new_folders if plan.manifest else [])
            if f not in result.removed_folders
        ]
        _write_back(plan.folder, failed, kept_folders)
    elif manifest_path.is_file():
        manifest_path.unlink()
    return result
