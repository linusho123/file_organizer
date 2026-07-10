"""Unit tests for the pure planning and execution logic."""

from pathlib import Path

import pytest

from file_organizer import organizer


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("run01.storx", "storx"),
        ("sample.mzML", "mzml"),
        ("calib.DMT", "dmt"),
        ("notes.txt", "txt"),
        ("README.md", "md"),
        ("archive.tar.gz", "gz"),
        ("Makefile", None),
        (".gitignore", None),
        ("weird.", None),
        (".hidden.txt", "txt"),
    ],
)
def test_get_extension(name, expected):
    assert organizer.get_extension(name) == expected


@pytest.mark.parametrize(
    ("ext", "expected"),
    [
        ("storx", "STORX_Files"),
        ("mzml", "MZML_Files"),
        ("txt", "TXT_Files"),
        (None, "NO_EXTENSION_Files"),
    ],
)
def test_folder_name_for(ext, expected):
    assert organizer.folder_name_for(ext) == expected


class TestResolveName:
    def test_no_conflict_keeps_name(self):
        assert organizer.resolve_name("report.txt", set()) == ("report.txt", False)

    def test_conflict_inserts_suffix_before_extension(self):
        assert organizer.resolve_name("report.txt", {"report.txt"}) == ("report_1.txt", True)

    def test_suffix_increments_until_free(self):
        taken = {"report.txt", "report_1.txt"}
        assert organizer.resolve_name("report.txt", taken) == ("report_2.txt", True)

    def test_extensionless_appends_suffix_at_end(self):
        assert organizer.resolve_name("Makefile", {"makefile"}) == ("Makefile_1", True)

    def test_comparison_is_case_insensitive_and_preserves_case(self):
        assert organizer.resolve_name("Report.TXT", {"report.txt"}) == ("Report_1.TXT", True)


class TestBuildPlan:
    def test_classifies_and_orders_case_insensitively(self, tmp_path):
        for name in ("b.txt", "A.txt", "c.storx"):
            (tmp_path / name).write_text("x")
        plan = organizer.build_plan(tmp_path)
        assert [m.source for m in plan.moves] == ["A.txt", "b.txt", "c.storx"]
        assert [m.dest_folder for m in plan.moves] == ["TXT_Files", "TXT_Files", "STORX_Files"]
        assert plan.new_folders == ["TXT_Files", "STORX_Files"]

    def test_skips_directories(self, tmp_path):
        (tmp_path / "sub").mkdir()
        (tmp_path / "sub" / "inner.txt").write_text("x")
        (tmp_path / "top.txt").write_text("x")
        plan = organizer.build_plan(tmp_path)
        assert [m.source for m in plan.moves] == ["top.txt"]
        assert [(s.name, s.reason) for s in plan.skipped] == [("sub", "directory")]

    def test_skips_symlinks(self, tmp_path):
        real = tmp_path / "real.txt"
        real.write_text("x")
        link = tmp_path / "link.txt"
        try:
            link.symlink_to(real)
        except OSError:
            pytest.skip("symlinks require elevated privileges on this platform")
        plan = organizer.build_plan(tmp_path)
        assert [m.source for m in plan.moves] == ["real.txt"]
        assert [(s.name, s.reason) for s in plan.skipped] == [("link.txt", "symlink")]

    def test_existing_type_folder_is_reused_not_created(self, tmp_path):
        (tmp_path / "TXT_Files").mkdir()
        (tmp_path / "new.txt").write_text("x")
        plan = organizer.build_plan(tmp_path)
        assert plan.new_folders == []
        assert plan.moves[0].dest_folder == "TXT_Files"

    def test_collision_with_existing_destination_file(self, tmp_path):
        dest = tmp_path / "TXT_Files"
        dest.mkdir()
        (dest / "report.txt").write_text("first")
        (tmp_path / "report.txt").write_text("second")
        plan = organizer.build_plan(tmp_path)
        move = plan.moves[0]
        assert move.final_name == "report_1.txt"
        assert move.renamed is True

    def test_empty_folder_yields_empty_plan(self, tmp_path):
        plan = organizer.build_plan(tmp_path)
        assert plan.moves == []
        assert plan.new_folders == []
        assert plan.skipped == []


class TestExecutePlan:
    def test_moves_files_and_creates_folders(self, tmp_path):
        (tmp_path / "a.txt").write_text("a")
        (tmp_path / "b.storx").write_text("b")
        plan = organizer.build_plan(tmp_path)
        result = organizer.execute_plan(plan)
        assert result.errors == []
        assert (tmp_path / "TXT_Files" / "a.txt").read_text() == "a"
        assert (tmp_path / "STORX_Files" / "b.storx").read_text() == "b"
        assert not (tmp_path / "a.txt").exists()

    def test_records_error_and_continues(self, tmp_path, monkeypatch):
        (tmp_path / "a.txt").write_text("a")
        (tmp_path / "b.txt").write_text("b")
        plan = organizer.build_plan(tmp_path)
        real_move = organizer.shutil.move

        def failing_move(src, dst):
            if Path(src).name == "a.txt":
                raise OSError("permission denied")
            return real_move(src, dst)

        monkeypatch.setattr(organizer.shutil, "move", failing_move)
        result = organizer.execute_plan(plan)
        assert [e.source for e in result.errors] == ["a.txt"]
        assert "permission denied" in result.errors[0].message
        assert [m.source for m in result.moved] == ["b.txt"]
        assert (tmp_path / "TXT_Files" / "b.txt").is_file()
        assert (tmp_path / "a.txt").is_file()

    def test_never_overwrites_on_collision(self, tmp_path):
        dest = tmp_path / "TXT_Files"
        dest.mkdir()
        (dest / "report.txt").write_text("first")
        (tmp_path / "report.txt").write_text("second")
        result = organizer.execute_plan(organizer.build_plan(tmp_path))
        assert result.errors == []
        assert (dest / "report.txt").read_text() == "first"
        assert (dest / "report_1.txt").read_text() == "second"
