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


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("TXT_Files", True),
        ("STORX_Files", True),
        ("TAR-GZ_Files", True),
        ("NO_EXTENSION_Files", True),
        ("My_Files", False),
        ("_Files", False),
        ("TXT_files", False),
        ("data", False),
    ],
)
def test_is_type_folder(name, expected):
    assert organizer.is_type_folder(name) is expected


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


class TestBuildPlanRecursive:
    def test_collects_nested_files_with_relative_paths(self, tmp_path):
        (tmp_path / "top.md").write_text("x")
        nested = tmp_path / "data"
        nested.mkdir()
        (nested / "notes.txt").write_text("x")
        deep = tmp_path / "a" / "b"
        deep.mkdir(parents=True)
        (deep / "deep.storx").write_text("x")
        plan = organizer.build_plan(tmp_path, recursive=True)
        assert [m.source for m in plan.moves] == ["a/b/deep.storx", "data/notes.txt", "top.md"]
        assert [m.dest_folder for m in plan.moves] == ["STORX_Files", "TXT_Files", "MD_Files"]
        assert plan.skipped == []

    def test_type_folders_are_not_traversed(self, tmp_path):
        dest = tmp_path / "TXT_Files"
        dest.mkdir()
        (dest / "report.txt").write_text("x")
        (tmp_path / "new.txt").write_text("x")
        plan = organizer.build_plan(tmp_path, recursive=True)
        assert [m.source for m in plan.moves] == ["new.txt"]
        assert [(s.name, s.reason) for s in plan.skipped] == [("TXT_Files", "type folder")]

    def test_nested_dir_with_type_folder_name_is_traversed(self, tmp_path):
        nested = tmp_path / "sub" / "TXT_Files"
        nested.mkdir(parents=True)
        (nested / "old.txt").write_text("x")
        plan = organizer.build_plan(tmp_path, recursive=True)
        assert [m.source for m in plan.moves] == ["sub/TXT_Files/old.txt"]

    def test_nested_manifest_is_skipped(self, tmp_path):
        inner = tmp_path / "inner"
        inner.mkdir()
        (inner / organizer.MANIFEST_NAME).write_text("{}")
        (inner / "notes.txt").write_text("x")
        plan = organizer.build_plan(tmp_path, recursive=True)
        assert [m.source for m in plan.moves] == ["inner/notes.txt"]
        assert [(s.name, s.reason) for s in plan.skipped] == [
            (f"inner/{organizer.MANIFEST_NAME}", "manifest")
        ]

    def test_cross_folder_collision_first_by_path_keeps_name(self, tmp_path):
        (tmp_path / "report.txt").write_text("top")
        sub = tmp_path / "sub"
        sub.mkdir()
        (sub / "report.txt").write_text("nested")
        plan = organizer.build_plan(tmp_path, recursive=True)
        assert [(m.source, m.final_name, m.renamed) for m in plan.moves] == [
            ("report.txt", "report.txt", False),
            ("sub/report.txt", "report_1.txt", True),
        ]

    def test_default_is_not_recursive(self, tmp_path):
        sub = tmp_path / "data"
        sub.mkdir()
        (sub / "notes.txt").write_text("x")
        plan = organizer.build_plan(tmp_path)
        assert plan.moves == []
        assert [(s.name, s.reason) for s in plan.skipped] == [("data", "directory")]

    def test_execute_moves_nested_file_and_leaves_empty_folder(self, tmp_path):
        sub = tmp_path / "data"
        sub.mkdir()
        (sub / "notes.txt").write_text("hello")
        result = organizer.execute_plan(organizer.build_plan(tmp_path, recursive=True))
        assert result.errors == []
        assert (tmp_path / "TXT_Files" / "notes.txt").read_text() == "hello"
        assert sub.is_dir()
        assert list(sub.iterdir()) == []


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
