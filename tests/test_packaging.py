"""Packaging and CI invariants (PRD section 16, FR-42..FR-48).

These guard the release pipeline: version single-sourcing, PyPI metadata
completeness, the license file, and the presence of the CI/publish workflows.
"""

from pathlib import Path

import pytest

import file_organizer

REPO_ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture(scope="module")
def project() -> dict:
    tomllib = pytest.importorskip("tomllib", reason="tomllib requires Python 3.11+")
    raw = (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    return tomllib.loads(raw)["project"]


def test_version_is_single_sourced(project):
    assert project["version"] == file_organizer.__version__


def test_distribution_name(project):
    assert project["name"] == "organize-by-extension"


def test_console_script_unchanged(project):
    assert project["scripts"]["file-organizer"] == "file_organizer.cli:entry"


def test_pypi_metadata_complete(project):
    for field in ("readme", "license", "authors", "keywords", "classifiers", "urls"):
        assert field in project, f"pyproject [project] lacks {field!r}"
    assert any("github.com/linusho123/file_organizer" in url for url in project["urls"].values())


def test_readme_referenced_by_metadata_exists(project):
    assert (REPO_ROOT / project["readme"]).is_file()


def test_license_file_present():
    text = (REPO_ROOT / "LICENSE").read_text(encoding="utf-8")
    assert "MIT License" in text


def test_build_and_twine_in_dev_extras(project):
    dev = project["optional-dependencies"]["dev"]
    assert "build" in dev
    assert "twine" in dev


def test_ci_workflow_covers_both_platforms_and_full_gate():
    text = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    needles = ("ubuntu-latest", "windows-latest", "ruff check", "ruff format --check", "pytest")
    for needle in needles:
        assert needle in text, f"ci.yml lacks {needle!r}"


def test_publish_workflow_uses_trusted_publishing():
    text = (REPO_ROOT / ".github" / "workflows" / "publish.yml").read_text(encoding="utf-8")
    assert "release" in text
    assert "id-token: write" in text
    assert "pypa/gh-action-pypi-publish" in text
