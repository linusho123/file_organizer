"""Unit tests for summary report formatting."""

from pathlib import Path

from file_organizer.organizer import MoveError, Plan, PlannedMove, RunResult, SkippedItem
from file_organizer.report import format_report


def make_plan(moves=(), new_folders=(), skipped=(), keep_structure=False, removable=()):
    return Plan(
        folder=Path("C:/data/workspace"),
        moves=list(moves),
        new_folders=list(new_folders),
        skipped=list(skipped),
        keep_structure=keep_structure,
        removable_source_dirs=list(removable),
    )


def test_real_run_report_full():
    plan = make_plan(
        moves=[PlannedMove("a.txt", "TXT_Files", "a.txt", renamed=False)],
        new_folders=["TXT_Files"],
        skipped=[SkippedItem("sub", "directory")],
    )
    result = RunResult(plan=plan, moved=list(plan.moves), errors=[])
    text = format_report(plan, result, dry_run=False)
    assert "DRY RUN" not in text
    assert "Organizing:" in text
    assert "a.txt  ->  TXT_Files/a.txt" in text
    assert "sub  (directory)" in text
    assert "Issues:\n  none" in text
    assert text.rstrip().endswith("Totals: 1 file moved, 1 folder created, 0 conflicts, 0 errors")


def test_dry_run_banner_is_first_line_and_uses_plan():
    plan = make_plan(
        moves=[PlannedMove("a.txt", "TXT_Files", "a.txt", renamed=False)],
        new_folders=["TXT_Files"],
    )
    text = format_report(plan, None, dry_run=True)
    assert text.splitlines()[0] == "DRY RUN - no changes made"
    assert "a.txt  ->  TXT_Files/a.txt" in text
    assert "Totals: 1 file moved, 1 folder created, 0 conflicts, 0 errors" in text


def test_conflict_and_error_lines():
    plan = make_plan(
        moves=[
            PlannedMove("report.txt", "TXT_Files", "report_1.txt", renamed=True),
            PlannedMove("data.dmt", "DMT_Files", "data.dmt", renamed=False),
        ],
        new_folders=["DMT_Files"],
    )
    result = RunResult(
        plan=plan,
        moved=[plan.moves[0]],
        errors=[MoveError("data.dmt", "permission denied")],
    )
    text = format_report(plan, result, dry_run=False)
    assert 'conflict: "report.txt" already existed in TXT_Files; moved as "report_1.txt"' in text
    assert 'error: could not move "data.dmt": permission denied' in text
    assert "Totals: 1 file moved, 1 folder created, 1 conflict, 1 error" in text


def test_empty_run_shows_none_sections_and_zero_totals():
    plan = make_plan()
    result = RunResult(plan=plan, moved=[], errors=[])
    text = format_report(plan, result, dry_run=False)
    assert "Folders created:\n  none" in text
    assert "Files moved:\n  none" in text
    assert "Skipped:\n  none" in text
    assert "Issues:\n  none" in text
    assert "Totals: 0 files moved, 0 folders created, 0 conflicts, 0 errors" in text


def test_keep_structure_report_has_source_folders_removed_section():
    plan = make_plan(
        moves=[PlannedMove("batch1/a.stori", "STORI_Files", "batch1/a.stori", renamed=False)],
        new_folders=["STORI_Files"],
        keep_structure=True,
        removable=["batch1"],
    )
    dry_text = format_report(plan, None, dry_run=True)
    assert "batch1/a.stori  ->  STORI_Files/batch1/a.stori" in dry_text
    assert "Source folders removed:\n  batch1" in dry_text
    result = RunResult(plan=plan, moved=list(plan.moves), errors=[], removed_source_dirs=["batch1"])
    real_text = format_report(plan, result, dry_run=False)
    assert "Source folders removed:\n  batch1" in real_text
    assert "Totals: 1 file moved, 1 folder created, 0 conflicts, 0 errors" in real_text


def test_flat_report_has_no_source_folders_section():
    plan = make_plan(moves=[PlannedMove("a.txt", "TXT_Files", "a.txt", renamed=False)])
    result = RunResult(plan=plan, moved=list(plan.moves), errors=[])
    assert "Source folders removed" not in format_report(plan, result, dry_run=False)


def test_undo_report_full():
    from file_organizer.report import format_undo_report
    from file_organizer.undo import PlannedRestore, UndoPlan, UndoResult

    plan = UndoPlan(
        folder=Path("C:/data/workspace"),
        restores=[PlannedRestore("a.txt", "TXT_Files", "a.txt", "a.txt", renamed=False)],
        missing=[],
        removable_folders=["TXT_Files"],
        manifest=None,
    )
    result = UndoResult(
        plan=plan, restored=list(plan.restores), removed_folders=["TXT_Files"], errors=[]
    )
    text = format_undo_report(plan, result, dry_run=False)
    assert "Undoing last run in:" in text
    assert "TXT_Files/a.txt  ->  a.txt" in text
    assert "Folders removed:" in text
    assert "Issues:\n  none" in text
    assert text.rstrip().endswith(
        "Totals: 1 file restored, 1 folder removed, 0 conflicts, 0 errors"
    )


def test_undo_report_dry_run_with_conflict_and_missing():
    from file_organizer.report import format_undo_report
    from file_organizer.undo import PlannedRestore, RecordedMove, UndoPlan

    plan = UndoPlan(
        folder=Path("C:/data/workspace"),
        restores=[PlannedRestore("a.txt", "TXT_Files", "a.txt", "a_1.txt", renamed=True)],
        missing=[RecordedMove("b.storx", "STORX_Files", "b.storx")],
        removable_folders=[],
        manifest=None,
    )
    text = format_undo_report(plan, None, dry_run=True)
    assert text.splitlines()[0] == "DRY RUN - no changes made"
    assert 'conflict: "a.txt" already existed; restored as "a_1.txt"' in text
    assert 'error: could not restore "STORX_Files/b.storx": file not found' in text
    assert "Totals: 1 file restored, 0 folders removed, 1 conflict, 1 error" in text


def test_sections_appear_in_required_order():
    plan = make_plan()
    result = RunResult(plan=plan, moved=[], errors=[])
    text = format_report(plan, result, dry_run=False)
    sections = ["Organizing:", "Folders created:", "Files moved:", "Skipped:", "Issues:", "Totals:"]
    positions = [text.index(s) for s in sections]
    assert positions == sorted(positions)
