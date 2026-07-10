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
