"""Unit tests for manifest persistence and undo planning/execution."""

import json

import pytest

from file_organizer import organizer, undo


def organize(folder, recursive=False, keep_structure=False):
    """Run a real organize pass and persist its manifest, like the CLI does."""
    plan = organizer.build_plan(folder, recursive=recursive, keep_structure=keep_structure)
    result = organizer.execute_plan(plan)
    undo.write_manifest(folder, result)
    return result


class TestManifestIO:
    def test_write_then_read_roundtrip(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        (tmp_path / "b.storx").write_text("x")
        organize(tmp_path)
        manifest = undo.read_manifest(tmp_path)
        assert manifest is not None
        assert [(m.source, m.dest_folder, m.final_name) for m in manifest.moves] == [
            ("a.txt", "TXT_Files", "a.txt"),
            ("b.storx", "STORX_Files", "b.storx"),
        ]
        assert manifest.new_folders == ["TXT_Files", "STORX_Files"]

    def test_read_returns_none_when_absent(self, tmp_path):
        assert undo.read_manifest(tmp_path) is None

    def test_corrupt_manifest_raises(self, tmp_path):
        (tmp_path / undo.MANIFEST_NAME).write_text("{not json")
        with pytest.raises(undo.ManifestError):
            undo.read_manifest(tmp_path)

    def test_zero_move_result_does_not_write(self, tmp_path):
        result = organizer.execute_plan(organizer.build_plan(tmp_path))
        undo.write_manifest(tmp_path, result)
        assert not (tmp_path / undo.MANIFEST_NAME).exists()

    def test_manifest_records_timestamp_and_version(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        raw = json.loads((tmp_path / undo.MANIFEST_NAME).read_text())
        assert raw["version"] == 1
        assert "created" in raw


class TestBuildUndoPlan:
    def test_plans_restores_and_removable_folders(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        manifest = undo.read_manifest(tmp_path)
        plan = undo.build_undo_plan(tmp_path, manifest)
        assert [(r.dest_folder, r.final_name, r.restore_name) for r in plan.restores] == [
            ("TXT_Files", "a.txt", "a.txt")
        ]
        assert plan.missing == []
        assert plan.removable_folders == ["TXT_Files"]

    def test_collision_at_top_level_gets_suffix(self, tmp_path):
        (tmp_path / "a.txt").write_text("original")
        organize(tmp_path)
        (tmp_path / "a.txt").write_text("newcomer")
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        restore = plan.restores[0]
        assert restore.restore_name == "a_1.txt"
        assert restore.renamed is True

    def test_missing_recorded_file_is_flagged(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        (tmp_path / "TXT_Files" / "a.txt").unlink()
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        assert plan.restores == []
        assert [(m.dest_folder, m.final_name) for m in plan.missing] == [("TXT_Files", "a.txt")]

    def test_folder_with_foreign_content_is_not_removable(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        (tmp_path / "TXT_Files" / "keepme.txt").write_text("user data")
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        assert plan.removable_folders == []

    def test_preexisting_folder_is_never_removable(self, tmp_path):
        (tmp_path / "TXT_Files").mkdir()
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        assert plan.removable_folders == []


class TestExecuteUndo:
    def test_restores_files_removes_folders_and_manifest(self, tmp_path):
        (tmp_path / "a.txt").write_text("hello")
        organize(tmp_path)
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (tmp_path / "a.txt").read_text() == "hello"
        assert not (tmp_path / "TXT_Files").exists()
        assert not (tmp_path / undo.MANIFEST_NAME).exists()
        assert result.removed_folders == ["TXT_Files"]

    def test_missing_file_is_an_error_and_manifest_kept(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        (tmp_path / "b.storx").write_text("x")
        organize(tmp_path)
        (tmp_path / "TXT_Files" / "a.txt").unlink()
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert len(result.errors) == 1
        assert "TXT_Files/a.txt" in result.errors[0].source
        assert (tmp_path / "b.storx").is_file()
        remaining = undo.read_manifest(tmp_path)
        assert remaining is not None
        assert [(m.dest_folder, m.final_name) for m in remaining.moves] == [("TXT_Files", "a.txt")]

    def test_move_failure_keeps_entry_in_manifest(self, tmp_path, monkeypatch):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))

        def failing_move(src, dst):
            raise OSError("locked")

        monkeypatch.setattr(undo.shutil, "move", failing_move)
        result = undo.execute_undo(plan)
        assert len(result.errors) == 1
        assert "locked" in result.errors[0].message
        remaining = undo.read_manifest(tmp_path)
        assert remaining is not None
        assert len(remaining.moves) == 1

    def test_nested_round_trip(self, tmp_path):
        sub = tmp_path / "data"
        sub.mkdir()
        (sub / "notes.txt").write_text("hello")
        organize(tmp_path, recursive=True)
        assert (tmp_path / "TXT_Files" / "notes.txt").is_file()
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (sub / "notes.txt").read_text() == "hello"
        assert not (tmp_path / "TXT_Files").exists()

    def test_restore_recreates_deleted_source_folder(self, tmp_path):
        sub = tmp_path / "data"
        sub.mkdir()
        (sub / "notes.txt").write_text("hello")
        organize(tmp_path, recursive=True)
        sub.rmdir()
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (sub / "notes.txt").read_text() == "hello"

    def test_restore_collision_resolved_inside_original_folder(self, tmp_path):
        sub = tmp_path / "data"
        sub.mkdir()
        (sub / "notes.txt").write_text("original")
        organize(tmp_path, recursive=True)
        (sub / "notes.txt").write_text("newcomer")
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        restore = plan.restores[0]
        assert restore.restore_name == "data/notes_1.txt"
        assert restore.renamed is True
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (sub / "notes.txt").read_text() == "newcomer"
        assert (sub / "notes_1.txt").read_text() == "original"

    def test_keep_structure_round_trip(self, tmp_path):
        batch1 = tmp_path / "batch1"
        batch1.mkdir()
        (batch1 / "a.stori").write_text("hello")
        batch2 = tmp_path / "batch2"
        batch2.mkdir()
        (batch2 / "c.stori").write_text("x")
        organize(tmp_path, recursive=True, keep_structure=True)
        assert not batch1.exists()
        assert (tmp_path / "STORI_Files" / "batch1" / "a.stori").is_file()
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (tmp_path / "batch1" / "a.stori").read_text() == "hello"
        assert (tmp_path / "batch2" / "c.stori").is_file()
        assert not (tmp_path / "STORI_Files").exists()
        assert not (tmp_path / undo.MANIFEST_NAME).exists()

    def test_created_type_folder_with_shells_is_removable(self, tmp_path):
        batch1 = tmp_path / "batch1"
        batch1.mkdir()
        (batch1 / "a.stori").write_text("x")
        organize(tmp_path, recursive=True, keep_structure=True)
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        assert plan.removable_folders == ["STORI_Files"]

    def test_preexisting_type_folder_is_kept_after_structured_undo(self, tmp_path):
        (tmp_path / "STORI_Files").mkdir()
        batch1 = tmp_path / "batch1"
        batch1.mkdir()
        (batch1 / "a.stori").write_text("x")
        organize(tmp_path, recursive=True, keep_structure=True)
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (tmp_path / "batch1" / "a.stori").is_file()
        assert (tmp_path / "STORI_Files").is_dir()

    def test_non_empty_created_folder_survives(self, tmp_path):
        (tmp_path / "a.txt").write_text("x")
        organize(tmp_path)
        (tmp_path / "TXT_Files" / "keepme.txt").write_text("user data")
        plan = undo.build_undo_plan(tmp_path, undo.read_manifest(tmp_path))
        result = undo.execute_undo(plan)
        assert result.errors == []
        assert (tmp_path / "TXT_Files" / "keepme.txt").is_file()
        assert (tmp_path / "a.txt").is_file()
