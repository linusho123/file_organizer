"""Unit tests for the CLI entry point: arguments, exit codes, wiring."""

import sys

import pytest

from file_organizer import cli, organizer


def test_missing_path_exits_2(tmp_path, capsys):
    code = cli.main([str(tmp_path / "does_not_exist")])
    assert code == 2
    err = capsys.readouterr().err
    assert "Error: path does not exist:" in err


def test_file_path_exits_2(tmp_path, capsys):
    target = tmp_path / "a.txt"
    target.write_text("x")
    code = cli.main([str(target)])
    assert code == 2
    assert "Error: path is not a directory:" in capsys.readouterr().err


def test_successful_run_exits_0_and_moves_files(tmp_path, capsys):
    (tmp_path / "a.txt").write_text("x")
    code = cli.main([str(tmp_path)])
    assert code == 0
    assert (tmp_path / "TXT_Files" / "a.txt").is_file()
    assert "Totals: 1 file moved" in capsys.readouterr().out


def test_empty_folder_exits_0(tmp_path, capsys):
    code = cli.main([str(tmp_path)])
    assert code == 0
    assert "Totals: 0 files moved" in capsys.readouterr().out


def test_dry_run_exits_0_and_changes_nothing(tmp_path, capsys):
    (tmp_path / "a.txt").write_text("x")
    code = cli.main([str(tmp_path), "--dry-run"])
    assert code == 0
    assert (tmp_path / "a.txt").is_file()
    assert not (tmp_path / "TXT_Files").exists()
    assert "DRY RUN - no changes made" in capsys.readouterr().out


def test_move_failure_exits_1_but_completes(tmp_path, capsys, monkeypatch):
    (tmp_path / "a.txt").write_text("x")

    def failing_move(src, dst):
        raise OSError("locked")

    monkeypatch.setattr(organizer.shutil, "move", failing_move)
    code = cli.main([str(tmp_path)])
    assert code == 1
    out = capsys.readouterr().out
    assert 'error: could not move "a.txt": locked' in out
    assert "1 error" in out


def test_recursive_flag_organizes_nested_files(tmp_path, capsys):
    sub = tmp_path / "data"
    sub.mkdir()
    (sub / "notes.txt").write_text("x")
    code = cli.main([str(tmp_path), "--recursive"])
    assert code == 0
    assert (tmp_path / "TXT_Files" / "notes.txt").is_file()
    assert "data/notes.txt  ->  TXT_Files/notes.txt" in capsys.readouterr().out


def test_recursive_dry_run_changes_nothing(tmp_path, capsys):
    sub = tmp_path / "data"
    sub.mkdir()
    (sub / "notes.txt").write_text("x")
    code = cli.main([str(tmp_path), "--recursive", "--dry-run"])
    assert code == 0
    assert (sub / "notes.txt").is_file()
    assert not (tmp_path / "TXT_Files").exists()


def test_help_mentions_recursive(capsys):
    with pytest.raises(SystemExit):
        cli.main(["--help"])
    assert "--recursive" in capsys.readouterr().out


def test_undo_without_manifest_exits_2(tmp_path, capsys):
    code = cli.main([str(tmp_path), "--undo"])
    assert code == 2
    assert "Error: no manifest found in:" in capsys.readouterr().err


def test_undo_with_corrupt_manifest_exits_2(tmp_path, capsys):
    from file_organizer import undo

    (tmp_path / undo.MANIFEST_NAME).write_text("{not json")
    code = cli.main([str(tmp_path), "--undo"])
    assert code == 2
    assert "Error: could not read manifest:" in capsys.readouterr().err


def test_organize_then_undo_round_trip(tmp_path, capsys):
    (tmp_path / "a.txt").write_text("hello")
    assert cli.main([str(tmp_path)]) == 0
    assert (tmp_path / "TXT_Files" / "a.txt").is_file()
    code = cli.main([str(tmp_path), "--undo"])
    assert code == 0
    assert (tmp_path / "a.txt").read_text() == "hello"
    assert not (tmp_path / "TXT_Files").exists()
    assert "Totals: 1 file restored, 1 folder removed" in capsys.readouterr().out


def test_undo_dry_run_changes_nothing(tmp_path, capsys):
    (tmp_path / "a.txt").write_text("x")
    assert cli.main([str(tmp_path)]) == 0
    code = cli.main([str(tmp_path), "--undo", "--dry-run"])
    assert code == 0
    assert (tmp_path / "TXT_Files" / "a.txt").is_file()
    assert "DRY RUN - no changes made" in capsys.readouterr().out


def test_undo_with_missing_file_exits_1(tmp_path, capsys):
    (tmp_path / "a.txt").write_text("x")
    assert cli.main([str(tmp_path)]) == 0
    (tmp_path / "TXT_Files" / "a.txt").unlink()
    code = cli.main([str(tmp_path), "--undo"])
    assert code == 1
    assert "error: could not restore" in capsys.readouterr().out


def test_version_flag_exits_0(capsys):
    with pytest.raises(SystemExit) as excinfo:
        cli.main(["--version"])
    assert excinfo.value.code == 0
    assert "file-organizer" in capsys.readouterr().out


def test_help_flag_exits_0(capsys):
    with pytest.raises(SystemExit) as excinfo:
        cli.main(["--help"])
    assert excinfo.value.code == 0
    out = capsys.readouterr().out
    assert "--dry-run" in out


def test_missing_argument_exits_2(capsys):
    with pytest.raises(SystemExit) as excinfo:
        cli.main([])
    assert excinfo.value.code == 2


def test_entry_exits_with_main_return_code(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(sys, "argv", ["file-organizer", str(tmp_path)])
    with pytest.raises(SystemExit) as excinfo:
        cli.entry()
    assert excinfo.value.code == 0
