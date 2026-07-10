"""Formatting of the end-of-run summary report (PRD FR-15/FR-16).

Output is deliberately ASCII-only so it renders safely on consoles whose
encoding cannot represent characters like arrows or em dashes.
"""

from __future__ import annotations

from file_organizer.organizer import Plan, RunResult


def _count(n: int, noun: str) -> str:
    return f"{n} {noun}" if n == 1 else f"{n} {noun}s"


def _section(lines: list[str], title: str, items: list[str]) -> None:
    lines.append(f"{title}:")
    if items:
        lines.extend(f"  {item}" for item in items)
    else:
        lines.append("  none")
    lines.append("")


def format_report(plan: Plan, result: RunResult | None, dry_run: bool) -> str:
    """Render the summary report; ``result`` is None for dry runs."""
    moves = plan.moves if result is None else result.moved
    errors = [] if result is None else result.errors

    lines: list[str] = []
    if dry_run:
        lines.append("DRY RUN - no changes made")
    lines.append(f"Organizing: {plan.folder}")
    lines.append("")

    _section(lines, "Folders created", list(plan.new_folders))
    _section(
        lines,
        "Files moved",
        [f"{m.source}  ->  {m.dest_folder}/{m.final_name}" for m in moves],
    )
    _section(lines, "Skipped", [f"{s.name}  ({s.reason})" for s in plan.skipped])

    conflicts = [m for m in moves if m.renamed]
    issues = [
        f'conflict: "{m.source}" already existed in {m.dest_folder}; moved as "{m.final_name}"'
        for m in conflicts
    ]
    issues.extend(f'error: could not move "{e.source}": {e.message}' for e in errors)
    _section(lines, "Issues", issues)

    lines.append(
        f"Totals: {_count(len(moves), 'file')} moved,"
        f" {_count(len(plan.new_folders), 'folder')} created,"
        f" {_count(len(conflicts), 'conflict')},"
        f" {_count(len(errors), 'error')}"
    )
    return "\n".join(lines)
