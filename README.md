# File Organizer

A Python CLI that organizes all files **directly inside a folder** into
subfolders named after each file's extension. A folder full of `.storx`,
`.mzml`, `.dmt`, `.txt`, and `.md` files becomes:

```text
my_folder/
├── STORX_Files/
├── MZML_Files/
├── DMT_Files/
├── TXT_Files/
└── MD_Files/
```

Nothing is ever overwritten or deleted: name collisions get a `_1`, `_2`, …
suffix, every run ends with a summary report (with an Issues section for
conflicts and errors), and the last run can be fully reversed with `--undo`.

The full specification lives in [PRD.md](PRD.md); acceptance criteria are
executable Gherkin scenarios in [`features/`](features/).

## Requirements

- Python 3.10+ (no runtime dependencies — standard library only)
- Windows, macOS, or Linux

## Installation

From the repository root:

```powershell
# create and use a virtual environment (recommended)
python -m venv .venv
.\.venv\Scripts\Activate.ps1        # PowerShell; on macOS/Linux: source .venv/bin/activate

# install the CLI
pip install .
```

For development (tests + linting), install editable with dev extras:

```powershell
pip install -e .[dev]
```

## Usage

```text
usage: file-organizer [-h] [--dry-run] [--undo] [--version] folder

positional arguments:
  folder       path to the folder whose top-level files will be organized

options:
  -h, --help   show help and exit
  --dry-run    preview all actions without changing the filesystem
  --undo       reverse the most recent organizing run recorded in the folder's manifest
  --version    print version and exit
```

### Examples

Preview what would happen (no changes made):

```powershell
file-organizer C:\Users\me\Downloads\instrument_dump --dry-run
```

Organize the folder:

```powershell
file-organizer C:\Users\me\Downloads\instrument_dump
```

Changed your mind? Reverse the last run:

```powershell
file-organizer C:\Users\me\Downloads\instrument_dump --undo
```

`python -m file_organizer <folder>` works too.

### Behavior in brief

- **Top level only.** Files inside existing subfolders are never touched;
  subfolders are listed as skipped in the report.
- **Extension = text after the last dot**, case-insensitive: `sample.mzML` →
  `MZML_Files/`, `archive.tar.gz` → `GZ_Files/`. Files with no extension
  (including dotfiles like `.gitignore`) go to `NO_EXTENSION_Files/`.
- **Collisions never overwrite.** `report.txt` arriving where one already
  exists becomes `report_1.txt`, and the conflict is listed under Issues.
- **Re-running is safe**: an already-organized folder is a no-op.
- **Undo manifest.** Each run that moves files writes
  `.file_organizer_manifest.json` into the folder. `--undo` moves every
  recorded file back, removes type folders the run created if they're now
  empty, then deletes the manifest. If some files can't be restored, the
  manifest keeps just those entries so the undo can be retried.
- **Exit codes**: `0` success, `1` completed with per-file errors, `2` invalid
  input (bad path / unreadable manifest / bad arguments).

## Development

The project is built spec-first: PRD → Gherkin feature files + tests +
linting → implementation phases. Tests must pass and lint must be clean
before any phase is considered done.

```powershell
# run everything: unit tests + all Gherkin scenarios, with >=90% coverage enforced
python -m pytest

# lint and formatting
ruff check .
ruff format --check .
```

Layout:

```text
├── PRD.md                  # spec: goals, FR-1..FR-27, build phases
├── features/               # Gherkin acceptance criteria (pytest-bdd)
├── src/file_organizer/
│   ├── cli.py              # argument parsing, exit codes
│   ├── organizer.py        # pure planning + execution of moves
│   ├── report.py           # summary report formatting
│   └── undo.py             # manifest persistence and undo
└── tests/                  # unit tests + BDD step definitions
```

## Iterations

| Version | Iteration | Contents |
|---|---|---|
| 0.1.0 | MVP | Organize top-level files into `<EXT>_Files` subfolders; collision auto-rename with suffixes; `--dry-run`; summary report with Issues section; exit codes 0/1/2. PRD §1–§12. |
| 0.2.0 | Iteration 2 — Undo | Move manifest written on every organizing run; `--undo` restores files (with collision suffixes), removes now-empty created folders, and consumes the manifest; partial failures keep a retryable manifest; `--undo --dry-run` preview. PRD §13. |

### Planned (backlog, PRD §11)

- `--recursive` mode pulling files out of nested subfolders
- Custom extension grouping (e.g. `.jpg` + `.png` → `Images_Files`)
- `--report-file <path>` to save the summary report
- `--only` / `--exclude` extension filters
- Watch mode / scheduled organizing
- Simple GUI or drag-and-drop wrapper
