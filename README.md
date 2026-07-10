# File Organizer

[![CI](https://github.com/linusho123/file_organizer/actions/workflows/ci.yml/badge.svg)](https://github.com/linusho123/file_organizer/actions/workflows/ci.yml)

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

### From PyPI (once the first release is published)

The package is published on PyPI as **`organize-by-extension`** (the names
`file-organizer` and `file-organizer-cli` were taken; the command you type is
still `file-organizer`). After the first GitHub release goes out:

```bash
# macOS / Linux
python3 -m pip install organize-by-extension
```

```powershell
# Windows (PowerShell)
python -m pip install organize-by-extension
```

Upgrade later with `pip install --upgrade organize-by-extension`. Same
no-venv, works-from-any-directory behavior as the install below.

### Global install from the repository — use from any directory, no venv

Install the CLI straight into your main Python. One command, run from
anywhere (adjust the path to wherever this repository lives):

```bash
# macOS / Linux
python3 -m pip install ~/gitrepos/file_organizer
```

```powershell
# Windows (PowerShell)
python -m pip install C:\Users\you\gitrepos\file_organizer
```

That's it. pip puts a `file-organizer` command into your Python installation's
scripts folder (`bin` on macOS/Linux, `Scripts` on Windows), which is on your
PATH — so from now on, in any terminal and from any directory, you can just
type:

```bash
# macOS / Linux
file-organizer ~/any/folder/you/want --dry-run
```

```powershell
# Windows (PowerShell)
file-organizer "C:\any\folder\you\want" --dry-run
```

No virtual environment, no activation, no `cd`-ing to the project first.

**Verify it worked:**

```bash
# macOS / Linux
file-organizer --version    # should print the current version
which file-organizer        # shows where the command lives
```

```powershell
# Windows (PowerShell)
file-organizer --version              # should print the current version
(Get-Command file-organizer).Source   # shows where the exe lives
```

**Upgrading after the tool gets new features:** just re-run the same install
command — pip rebuilds from the repository folder and replaces the installed
copy in place. Already-open terminals pick up the new version on their next
invocation; no restart needed.

**Troubleshooting — `file-organizer: command not found` / not recognized:**

1. Open a **new** terminal first. PATH changes (e.g. from installing Python
   itself) only reach terminals opened afterwards.
2. Still not found? Print your Python scripts folder and check it's on PATH:
   `python3 -c "import sysconfig; print(sysconfig.get_path('scripts'))"`
   (use `python` instead of `python3` on Windows). That folder must be on your
   PATH — check with `echo $PATH` on macOS/Linux, or
   `[Environment]::GetEnvironmentVariable("Path", "User")` on Windows.
3. As a last resort, running it as a module always works from anywhere without
   relying on PATH: `python3 -m file_organizer ~/any/folder` (or `python -m
   file_organizer "C:\any\folder"` on Windows).

### Development setup (only for working on the tool itself)

The `.venv` in this repository is **not needed to use the tool** — it exists
so tests and linting run against pinned dev dependencies. To contribute:

```bash
# macOS / Linux
python3 -m venv .venv
source .venv/bin/activate
pip install -e '.[dev]'
```

```powershell
# Windows (PowerShell)
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -e .[dev]
```

## Usage

```text
usage: file-organizer [-h] [--dry-run] [--recursive] [--keep-structure] [--undo] [--version] folder

positional arguments:
  folder            path to the folder whose top-level files will be organized

options:
  -h, --help        show help and exit
  --dry-run         preview all actions without changing the filesystem
  --recursive       also organize files inside nested subfolders (type folders are never traversed)
  --keep-structure  with --recursive: mirror each file's source subpath inside its type folder
                    and remove source folders emptied by the run
  --undo            reverse the most recent organizing run recorded in the folder's manifest
  --version         print version and exit
```

### Examples

These use macOS/Linux paths; on Windows swap in a `C:\...` path (e.g.
`file-organizer "C:\Users\me\Downloads\instrument_dump" --dry-run`).

Preview what would happen (no changes made):

```bash
file-organizer ~/Downloads/instrument_dump --dry-run
```

Organize the folder:

```bash
file-organizer ~/Downloads/instrument_dump
```

Also pull files out of nested subfolders (they move into the top-level type
folders; emptied subfolders are left in place):

```bash
file-organizer ~/Downloads/instrument_dump --recursive
```

Transport whole subfolders into the type folders, keeping their internal
organization (three subfolders of `.stori` files become
`STORI_Files/batch1/…`, `STORI_Files/batch2/…`, `STORI_Files/batch3/…`;
emptied source folders are removed):

```bash
file-organizer ~/Downloads/instrument_dump --recursive --keep-structure
```

Changed your mind? Reverse the last run:

```bash
file-organizer ~/Downloads/instrument_dump --undo
```

`python3 -m file_organizer <folder>` works too (`python` on Windows).

### Behavior in brief

- **Top level only by default.** Files inside existing subfolders are never
  touched unless you pass `--recursive`; subfolders are listed as skipped in
  the report. With `--recursive`, files at any depth move into the top-level
  type folders (which are themselves never traversed), and undo puts them
  back in their original subfolders — recreating any that were deleted.
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

```bash
# run everything: unit tests + all Gherkin scenarios, with >=90% coverage enforced
python -m pytest

# lint and formatting
ruff check .
ruff format --check .
```

Layout:

```text
├── PRD.md                  # spec: goals, FR-1..FR-48, build phases
├── .github/workflows/      # CI (every push) and PyPI publish (on release)
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
| 0.3.0 | Iteration 3 — Recursive | `--recursive` organizes files at any depth into top-level type folders; type folders are destinations, never traversed; nested manifests protected; deterministic relative-path ordering; undo restores nested files to their original folders, recreating deleted ones. PRD §14. |
| 0.4.0 | Iteration 4 — Keep structure | `--keep-structure` (with `--recursive`) mirrors each file's source subpath inside its type folder instead of flattening; mixed folders split by type; collisions resolved per destination folder; emptied source folders removed (reported in a `Source folders removed` section); undo restores the exact original tree and prunes empty shells. PRD §15. |
| 0.5.0 | Iteration 5 — CI & PyPI packaging | GitHub Actions CI (lint + full suite on Ubuntu and Windows, Python 3.10/3.13); complete PyPI metadata as `organize-by-extension` with MIT license; release-triggered publish workflow via PyPI trusted publishing. No CLI changes. PRD §16. |

### Planned (backlog, PRD §11)

- Custom extension grouping (e.g. `.jpg` + `.png` → `Images_Files`)
- `--report-file <path>` to save the summary report
- `--only` / `--exclude` extension filters
- Watch mode / scheduled organizing
- Simple GUI or drag-and-drop wrapper

## License

MIT — see [LICENSE](LICENSE).
