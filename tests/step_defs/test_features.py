"""pytest-bdd step definitions binding the Gherkin features to the CLI."""

from pathlib import Path

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

from file_organizer import cli

FEATURES_DIR = Path(__file__).resolve().parents[2] / "features"
scenarios(str(FEATURES_DIR))

SECTION_ORDER = ["Folders created:", "Files moved:", "Skipped:", "Issues:", "Totals:"]


@pytest.fixture
def ctx(tmp_path):
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    return {"workspace": workspace, "target": workspace}


def _snapshot(root: Path) -> dict:
    state = {}
    for path in sorted(root.rglob("*")):
        rel = path.relative_to(root).as_posix()
        state[rel] = path.read_bytes() if path.is_file() else "<dir>"
    return state


def _run(ctx, capsys, extra=()) -> None:
    exit_code = cli.main([str(ctx["target"]), *extra])
    captured = capsys.readouterr()
    ctx["exit_code"] = exit_code
    ctx["stdout"] = captured.out
    ctx["stderr"] = captured.err


# --- Given -------------------------------------------------------------


@given("the target path does not exist")
def target_missing(ctx, tmp_path):
    ctx["target"] = tmp_path / "does_not_exist"


@given("the target path is a file")
def target_is_file(ctx, tmp_path):
    target = tmp_path / "target.txt"
    target.write_text("not a folder")
    ctx["target"] = target


@given(parsers.re(r'the workspace contains a file named "(?P<name>[^"]+)"$'))
def workspace_file(ctx, name):
    (ctx["workspace"] / name).write_text(f"content-of-{name}")


@given(
    parsers.re(
        r'the workspace contains a file named "(?P<name>[^"]+)"'
        r' with content "(?P<content>[^"]*)"$'
    )
)
def workspace_file_with_content(ctx, name, content):
    (ctx["workspace"] / name).write_text(content)


@given(
    parsers.re(
        r'the workspace contains a subfolder named "(?P<folder>[^"]+)"'
        r' containing a file named "(?P<name>[^"]+)"$'
    )
)
def workspace_subfolder_file(ctx, folder, name):
    sub = ctx["workspace"] / folder
    sub.mkdir(exist_ok=True)
    (sub / name).write_text(f"content-of-{name}")


@given(
    parsers.re(
        r'the workspace contains a subfolder named "(?P<folder>[^"]+)"'
        r' containing a file named "(?P<name>[^"]+)" with content "(?P<content>[^"]*)"$'
    )
)
def workspace_subfolder_file_with_content(ctx, folder, name, content):
    sub = ctx["workspace"] / folder
    sub.mkdir(exist_ok=True)
    (sub / name).write_text(content)


# --- When --------------------------------------------------------------


@when("I run the organizer on the target")
def run_organizer(ctx, capsys):
    _run(ctx, capsys)


@when("I run the organizer on the target again")
def run_organizer_again(ctx, capsys):
    _run(ctx, capsys)


@when("I run the organizer on the target with --dry-run")
def run_organizer_dry(ctx, capsys):
    ctx["snapshot"] = _snapshot(ctx["workspace"])
    _run(ctx, capsys, ["--dry-run"])


@given(parsers.re(r'the workspace contains a nested file named "(?P<rel>[^"]+)"$'))
def workspace_nested_file(ctx, rel):
    path = ctx["workspace"] / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"content-of-{rel}")


@given(
    parsers.re(
        r'the workspace contains a nested file named "(?P<rel>[^"]+)"'
        r' with content "(?P<content>[^"]*)"$'
    )
)
def workspace_nested_file_with_content(ctx, rel, content):
    path = ctx["workspace"] / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)


@given(parsers.re(r'the workspace contains an empty subfolder named "(?P<rel>[^"]+)"$'))
def workspace_empty_subfolder(ctx, rel):
    (ctx["workspace"] / rel).mkdir(parents=True, exist_ok=True)


@when("I run the organizer on the target with --recursive")
def run_organizer_recursive(ctx, capsys):
    _run(ctx, capsys, ["--recursive"])


@when("I run the organizer on the target with --recursive and --keep-structure")
def run_organizer_keep_structure(ctx, capsys):
    _run(ctx, capsys, ["--recursive", "--keep-structure"])


@when("I run the organizer on the target with --keep-structure only")
def run_organizer_keep_structure_only(ctx, capsys):
    _run(ctx, capsys, ["--keep-structure"])


@when("I run the organizer on the target with --recursive, --keep-structure and --dry-run")
def run_organizer_keep_structure_dry(ctx, capsys):
    ctx["snapshot"] = _snapshot(ctx["workspace"])
    _run(ctx, capsys, ["--recursive", "--keep-structure", "--dry-run"])


@when("I run the organizer on the target with --recursive and --dry-run")
def run_organizer_recursive_dry(ctx, capsys):
    ctx["snapshot"] = _snapshot(ctx["workspace"])
    _run(ctx, capsys, ["--recursive", "--dry-run"])


@when(parsers.re(r'the folder "(?P<rel>[^"]+)" is deleted from the workspace$'))
def workspace_folder_deleted(ctx, rel):
    (ctx["workspace"] / rel).rmdir()


@when("I run the organizer on the target with --undo")
def run_organizer_undo(ctx, capsys):
    _run(ctx, capsys, ["--undo"])


@when("I run the organizer on the target with --undo and --dry-run")
def run_organizer_undo_dry(ctx, capsys):
    _run(ctx, capsys, ["--undo", "--dry-run"])


@when(
    parsers.re(
        r'the workspace gains a file named "(?P<name>[^"]+)" with content "(?P<content>[^"]*)"$'
    )
)
def workspace_gains_file(ctx, name, content):
    (ctx["workspace"] / name).write_text(content)


@when(parsers.re(r'the file "(?P<rel>[^"]+)" is deleted from the workspace$'))
def workspace_file_deleted(ctx, rel):
    (ctx["workspace"] / rel).unlink()


# --- Then --------------------------------------------------------------


@then(parsers.parse("the exit code is {code:d}"))
def check_exit_code(ctx, code):
    assert ctx["exit_code"] == code, (
        f"expected exit code {code}, got {ctx['exit_code']}\n"
        f"stdout:\n{ctx['stdout']}\nstderr:\n{ctx['stderr']}"
    )


@then(parsers.re(r'stderr contains "(?P<text>[^"]+)"$'))
def check_stderr_contains(ctx, text):
    assert text in ctx["stderr"], f"missing {text!r} in stderr:\n{ctx['stderr']}"


@then(parsers.re(r'the report contains "(?P<text>[^"]+)"$'))
def check_report_contains(ctx, text):
    assert text in ctx["stdout"], f"missing {text!r} in report:\n{ctx['stdout']}"


@then(parsers.re(r'the report does not contain "(?P<text>[^"]+)"$'))
def check_report_not_contains(ctx, text):
    assert text not in ctx["stdout"], f"unexpected {text!r} in report:\n{ctx['stdout']}"


@then(parsers.re(r'the workspace contains a folder named "(?P<name>[^"]+)"$'))
def check_folder_exists(ctx, name):
    assert (ctx["workspace"] / name).is_dir(), f"folder {name!r} missing"


@then(parsers.re(r'the workspace does not contain a folder named "(?P<name>[^"]+)"$'))
def check_folder_not_exists(ctx, name):
    assert not (ctx["workspace"] / name).exists(), f"folder {name!r} should not exist"


@then(parsers.re(r'the file "(?P<rel>[^"]+)" exists in the workspace$'))
def check_file_exists(ctx, rel):
    assert (ctx["workspace"] / rel).is_file(), f"file {rel!r} missing"


@then(parsers.re(r'the file "(?P<rel>[^"]+)" does not exist in the workspace$'))
def check_file_not_exists(ctx, rel):
    assert not (ctx["workspace"] / rel).exists(), f"file {rel!r} should not exist"


@then(parsers.re(r'the file "(?P<rel>[^"]+)" in the workspace has content "(?P<content>[^"]*)"$'))
def check_file_content(ctx, rel, content):
    path = ctx["workspace"] / rel
    assert path.is_file(), f"file {rel!r} missing"
    assert path.read_text() == content


@then("the workspace is unchanged")
def check_workspace_unchanged(ctx):
    assert _snapshot(ctx["workspace"]) == ctx["snapshot"]


@then("the report sections appear in order")
def check_sections_in_order(ctx):
    out = ctx["stdout"]
    positions = []
    for section in SECTION_ORDER:
        assert section in out, f"missing section {section!r} in report:\n{out}"
        positions.append(out.index(section))
    assert positions == sorted(positions), f"sections out of order in report:\n{out}"


@then("the Issues section shows none")
def check_issues_none(ctx):
    assert "Issues:\n  none" in ctx["stdout"], f"Issues section not empty:\n{ctx['stdout']}"


# --- Then: packaging & CI (repository-level checks) ---------------------

REPO_ROOT = FEATURES_DIR.parent


def _pyproject() -> dict:
    tomllib = pytest.importorskip("tomllib", reason="tomllib requires Python 3.11+")
    return tomllib.loads((REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8"))


@then("the pyproject version matches the package version")
def check_pyproject_version():
    import file_organizer

    assert _pyproject()["project"]["version"] == file_organizer.__version__


@then(parsers.re(r'the pyproject declares the distribution name "(?P<name>[^"]+)"$'))
def check_distribution_name(name):
    assert _pyproject()["project"]["name"] == name


@then(parsers.re(r'the pyproject declares the metadata field "(?P<field>[^"]+)"$'))
def check_metadata_field(field):
    assert field in _pyproject()["project"], f"pyproject [project] lacks {field!r}"


@then(parsers.re(r'the pyproject declares the console script "(?P<script>[^"]+)"$'))
def check_console_script(script):
    assert script in _pyproject()["project"]["scripts"]


@then(parsers.re(r'the repository file "(?P<rel>[^"]+)" exists$'))
def check_repo_file_exists(rel):
    assert (REPO_ROOT / rel).is_file(), f"repository file {rel!r} missing"


@then(parsers.re(r'the repository file "(?P<rel>[^"]+)" contains "(?P<text>[^"]+)"$'))
def check_repo_file_contains(rel, text):
    path = REPO_ROOT / rel
    assert path.is_file(), f"repository file {rel!r} missing"
    assert text in path.read_text(encoding="utf-8"), f"{rel!r} does not contain {text!r}"
