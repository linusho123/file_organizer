# PRD — File Organizer CLI (MVP)

| | |
|---|---|
| **Project** | `file_organizer` |
| **Version** | 0.1 (MVP spec) |
| **Date** | 2026-07-10 |
| **Status** | Draft — approved decisions from stakeholder Q&A baked in |
| **Stack** | Python 3.10+ CLI, standard library only |

---

## 1. Overview

File Organizer is a command-line tool that takes the path of a folder and organizes
every file **directly inside that folder** (top level only) into subfolders named
after each file's extension. For example, a folder containing `.storx`, `.dmt`,
`.mzml`, `.txt`, and `.md` files is reorganized into `STORX_Files/`, `DMT_Files/`,
`MZML_Files/`, `TXT_Files/`, and `MD_Files/` subfolders, with each file moved into
its matching subfolder. At the end of every run the tool prints a summary report,
including a dedicated **Issues** section for name conflicts and errors.

## 2. Goals

- **G1** — Organize all top-level files in a user-supplied folder into
  extension-based subfolders with a single command.
- **G2** — Never lose or overwrite data: collisions are resolved by renaming, never
  by overwriting or silently skipping.
- **G3** — Be safe to explore: a `--dry-run` mode previews every action without
  touching the filesystem.
- **G4** — Be transparent: every run ends with a human-readable summary report of
  what was moved, created, renamed, skipped, and any errors.
- **G5** — Be verifiable: behavior is specified in Gherkin feature files, enforced
  by unit tests (pytest) and linting (ruff), and built in defined phases.

## 3. Non-Goals (MVP)

- **NG1** — No recursion: files inside existing subfolders are not touched.
- **NG2** — No GUI; CLI only.
- **NG3** — No undo/rollback command (future iteration).
- **NG4** — No content-based file-type detection (MIME sniffing); classification is
  by filename extension only.
- **NG5** — No configuration file, custom folder-name mapping, or grouping of
  related extensions (e.g., `.jpg` + `.png` → `Images`); every distinct extension
  gets its own folder.
- **NG6** — No watching/daemon mode; each run is a one-shot operation.
- **NG7** — No third-party runtime dependencies (dev dependencies like pytest and
  ruff are allowed).

## 4. Users & Primary Use Case

A single user (initially: lab/data-tooling context with instrument output files
such as `.storx`, `.dmt`, `.mzml` mixed with notes and docs) who periodically dumps
many files into one folder and wants them sorted by type in one command.

## 5. Definitions

| Term | Definition |
|---|---|
| **Input folder** | The folder path passed by the user; the only folder whose direct children are processed. |
| **Extension** | The substring after the **last** dot in a filename, excluding a leading dot. `data.tar.gz` → `gz`. Matching is **case-insensitive** (`.TXT` ≡ `.txt`). |
| **Extensionless file** | A file with no dot after its first character. Includes dotfiles like `.gitignore` (a leading dot is part of the name, not an extension marker). |
| **Type folder** | Destination subfolder, named `<EXT_UPPERCASE>_Files` (e.g., `MZML_Files`) or `NO_EXTENSION_Files`. |
| **Collision** | A file being moved into a type folder that already contains a file with the same name (case-insensitive comparison, since Windows filesystems are case-insensitive). |

## 6. Functional Requirements

### 6.1 Input & validation

- **FR-1** — The CLI accepts exactly one required positional argument: the input
  folder path. Absolute and relative paths are both accepted; relative paths are
  resolved against the current working directory.
- **FR-2** — If the path does not exist, the tool prints
  `Error: path does not exist: <path>` to stderr and exits with code `2`. Nothing
  is created or moved.
- **FR-3** — If the path exists but is not a directory (e.g., it is a file), the
  tool prints `Error: path is not a directory: <path>` to stderr and exits with
  code `2`. Nothing is created or moved.
- **FR-4** — If the folder contains zero top-level files, the tool prints the
  summary report showing 0 files processed and exits with code `0`.

### 6.2 Classification

- **FR-5** — Only **regular files that are direct children** of the input folder
  are processed. Subdirectories (and everything inside them) and symbolic links
  are never moved, renamed, or deleted.
- **FR-6** — Each file is classified by its extension as defined in §5. The
  destination type folder name is the extension uppercased plus the literal
  suffix `_Files`.

  | Filename | Extension | Destination folder |
  |---|---|---|
  | `run01.storx` | `storx` | `STORX_Files` |
  | `sample.mzML` | `mzml` | `MZML_Files` |
  | `calib.DMT` | `dmt` | `DMT_Files` |
  | `notes.txt` | `txt` | `TXT_Files` |
  | `README.md` | `md` | `MD_Files` |
  | `archive.tar.gz` | `gz` | `GZ_Files` |
  | `Makefile` | *(none)* | `NO_EXTENSION_Files` |
  | `.gitignore` | *(none)* | `NO_EXTENSION_Files` |
  | `weird.` (trailing dot) | *(none)* | `NO_EXTENSION_Files` |

- **FR-7** — A type folder is created only if at least one file classifies into
  it. Folder creation is idempotent: if `TXT_Files` already exists (e.g., from a
  previous run), it is reused, not duplicated or errored on.
- **FR-8** — Existing type folders are themselves directories and therefore, per
  FR-5, are never processed as files. Re-running the tool on an already-organized
  folder is a no-op that reports 0 files moved and exits `0`.

### 6.3 Moving & collision handling

- **FR-9** — Each classified file is moved (not copied) into its type folder via
  an atomic same-volume rename where possible.
- **FR-10** — On collision (§5), the incoming file is renamed by appending the
  lowest available integer suffix before the extension:
  `report.txt` → `report_1.txt`; if `report_1.txt` also exists → `report_2.txt`,
  and so on. For extensionless files the suffix is appended to the end:
  `Makefile` → `Makefile_1`.
- **FR-11** — Every collision-rename is recorded and listed in the **Issues**
  section of the summary report (original name → final name → destination),
  alongside errors. Collisions do **not** cause a non-zero exit code.
- **FR-12** — If moving a single file fails (e.g., permission denied, file locked
  by another process), the tool records the error, **continues with the remaining
  files**, lists the failure in the Issues section, and exits with code `1`.

### 6.4 Dry-run mode

- **FR-13** — With `--dry-run`, the tool performs classification and collision
  simulation but makes **zero filesystem changes**: no folders created, no files
  moved or renamed.
- **FR-14** — Dry-run output uses the same report format, clearly prefixed with a
  `DRY RUN — no changes made` banner, and shows exactly what a real run would do,
  including would-be collision renames in the Issues section.

### 6.5 Summary report

- **FR-15** — Every run (real or dry) ends with a report printed to stdout with
  these sections, in order:

  1. **Header** — mode (`DRY RUN` banner if applicable) and the resolved input path.
  2. **Folders created** — each type folder newly created this run.
  3. **Files moved** — each move as `<name>  →  <TypeFolder>/<final name>`.
  4. **Skipped** — subdirectories and symlinks found at top level (name + reason).
  5. **Issues** — collision renames (`conflict: report.txt existed in TXT_Files;
     moved as report_1.txt`) and errors (`error: could not move data.dmt —
     permission denied`). Prints `none` when empty.
  6. **Totals** — `N files moved, M folders created, K conflicts, E errors`.

- **FR-16** — Example report:

  ```text
  Organizing: C:\Users\linus\Downloads\instrument_dump

  Folders created:
    STORX_Files
    TXT_Files

  Files moved:
    run01.storx      →  STORX_Files/run01.storx
    run02.storx      →  STORX_Files/run02.storx
    report.txt       →  TXT_Files/report.txt
    report.txt (2nd) →  TXT_Files/report_1.txt

  Skipped:
    old_backups/  (directory)

  Issues:
    conflict: "report.txt" already existed in TXT_Files — moved as "report_1.txt"

  Totals: 4 files moved, 2 folders created, 1 conflict, 0 errors
  ```

### 6.6 CLI interface

- **FR-17** — Invocation and flags:

  ```text
  usage: file-organizer [-h] [--dry-run] [--version] folder

  positional arguments:
    folder       path to the folder whose top-level files will be organized

  options:
    -h, --help   show help and exit
    --dry-run    preview all actions without changing the filesystem
    --version    print version and exit
  ```

- **FR-18** — Exit codes: `0` success (including no-op and dry runs),
  `1` completed but one or more files failed to move, `2` invalid input
  (bad path, not a directory, bad arguments).

## 7. Non-Functional Requirements

- **NFR-1** — Python 3.10+; runtime uses only the standard library (`argparse`,
  `pathlib`, `shutil`, `sys`).
- **NFR-2** — Cross-platform: Windows, macOS, Linux. All path handling via
  `pathlib`; no shell-outs.
- **NFR-3** — Deterministic ordering: files are processed in case-insensitive
  alphabetical order so reports and collision suffixes are reproducible.
- **NFR-4** — Performance: a folder of 10,000 files completes in under 30 seconds
  on commodity hardware (moves are renames, so this is generous).
- **NFR-5** — The tool never deletes files or folders under any circumstance.

## 8. Quality Plan

| Concern | Tool / approach |
|---|---|
| Acceptance criteria | Gherkin `.feature` files (one per FR group: validation, classification, collisions, dry-run, report), executed with **pytest-bdd** |
| Unit tests | **pytest**, with `tmp_path` fixtures for real-filesystem tests; target ≥ 90% line coverage on the core module |
| Linting & formatting | **ruff** (`ruff check` + `ruff format`) — Python equivalent of the originally proposed oxlint/eslint, which are JavaScript-only |
| CI gate (definition of done) | `ruff check` clean, `ruff format --check` clean, all pytest + pytest-bdd scenarios pass |

Proposed project layout:

```text
file_organizer/
├── PRD.md
├── pyproject.toml            # project metadata, ruff + pytest config
├── src/
│   └── file_organizer/
│       ├── __init__.py
│       ├── __main__.py       # python -m file_organizer
│       ├── cli.py            # argparse, exit codes
│       ├── organizer.py      # pure planning logic (classify, collision plan)
│       └── report.py         # report formatting
├── features/
│   ├── input_validation.feature
│   ├── classification.feature
│   ├── collisions.feature
│   ├── dry_run.feature
│   └── report.feature
└── tests/
    ├── test_organizer.py
    ├── test_report.py
    ├── test_cli.py
    └── step_defs/            # pytest-bdd step definitions
```

Design note: `organizer.py` produces a **plan** (list of intended moves/renames)
separately from executing it. Dry-run prints the plan; real runs execute it. This
makes the core logic pure and trivially unit-testable.

## 9. Build Phases

1. **Phase 0 — Scaffolding**: repo init (done), `pyproject.toml`, package skeleton,
   ruff + pytest wired up, empty CLI that prints help. *Gate:* lint passes,
   `file-organizer --help` works.
2. **Phase 1 — Gherkin feature files**: write all `.feature` files with acceptance
   criteria and examples derived from §6 (before implementation). *Gate:* features
   reviewed and agreed; scenarios runnable (failing) via pytest-bdd.
3. **Phase 2 — Core planning logic**: `organizer.py` classification + collision
   planning, fully unit-tested. *Gate:* classification/collision unit tests green.
4. **Phase 3 — Execution + report**: move execution, error capture, `report.py`,
   `--dry-run`. *Gate:* dry-run and report scenarios green.
5. **Phase 4 — CLI hardening**: argument validation, exit codes, `--version`,
   end-to-end tests. *Gate:* **all** Gherkin scenarios + unit tests pass, ruff
   clean, coverage ≥ 90%. → **This is the MVP.**

## 10. Acceptance Criteria (top-level)

The MVP is done when all of the following hold on Windows (primary) and at least
one POSIX platform (CI):

- **AC-1** — Given a folder with `.storx`, `.dmt`, `.mzml`, `.txt`, `.md` files,
  running the tool produces `STORX_Files`, `DMT_Files`, `MZML_Files`, `TXT_Files`,
  `MD_Files` containing exactly the corresponding files, and the original folder's
  top level contains only those subfolders.
- **AC-2** — Running the tool twice in a row is safe: the second run moves 0 files
  and exits `0`.
- **AC-3** — A collision results in a `_N` suffixed file, both files' contents
  intact, and a conflict entry in the report's Issues section.
- **AC-4** — `--dry-run` leaves the folder bit-for-bit unchanged while printing
  the full would-be report.
- **AC-5** — Invalid paths exit `2` with a clear stderr message; per-file move
  failures exit `1` after processing everything else.
- **AC-6** — `ruff check`, `ruff format --check`, and the full pytest suite
  (units + BDD scenarios) all pass.

## 11. Future Iterations (post-MVP backlog, not in scope)

- ~~Recursive mode (`--recursive`) pulling files out of nested subfolders.~~
  **Implemented in Iteration 3 — see §14.**
- Custom mapping config (e.g., group `.jpg`/`.png` → `Images_Files`; rename schemes).
- ~~Undo: write a manifest of moves each run; `--undo` replays it in reverse.~~
  **Implemented in Iteration 2 — see §13.**
- `--report-file <path>` to save the summary report to disk.
- Filters: `--only ext1,ext2` / `--exclude ext1,ext2`.
- Watch mode / scheduled organizing.
- Simple GUI or drag-and-drop wrapper.

## 12. Open Questions

- Should the report also be written to a log file by default (currently: stdout
  only, `--report-file` deferred to backlog)?
- Should hidden/system files (beyond dotfiles) on Windows be skipped or organized?
  MVP treats them like any other file.

---

## 13. Iteration 2 — Undo (v0.2.0)

Reverses the most recent organizing run using a manifest written by every run
that moves at least one file.

### 13.1 Manifest

- **FR-19** — Every real (non-dry) run that moves at least one file writes
  `.file_organizer_manifest.json` into the input folder, overwriting any
  previous manifest. It records a format version, an ISO-8601 timestamp, every
  executed move (original name, type folder, final name), and the type folders
  created by that run. Runs that move zero files leave an existing manifest
  untouched; dry runs never write one.
- **FR-20** — The manifest file itself is never organized: it is skipped and
  listed in the report's Skipped section with reason `(manifest)`.

### 13.2 Undo behavior

- **FR-21** — `--undo` reads the manifest and reverses the recorded run: each
  recorded file is moved from `<TypeFolder>/<final name>` back to its original
  top-level name, in recorded order.
- **FR-22** — If the original name is already taken at the top level, the file
  is restored with the lowest free `_N` suffix and the rename is reported as a
  conflict in the Issues section; the exit code stays 0.
- **FR-23** — Recorded files that no longer exist at their recorded location
  are reported as errors in Issues; all other files are still restored, and
  the run exits 1.
- **FR-24** — After restoring, type folders that the recorded run created are
  removed only if empty. Folders holding any other content are left in place.
- **FR-25** — On full success the manifest is deleted. If any restore failed,
  the manifest is rewritten to contain only the entries that were not restored
  so the undo can be retried; folders already removed are dropped from it.
- **FR-26** — `--undo --dry-run` previews the full undo report (including
  would-be conflicts and missing-file errors) with zero filesystem changes.
- **FR-27** — `--undo` with no manifest exits 2 with
  `Error: no manifest found in: <path>`. An unreadable or corrupt manifest
  exits 2 with `Error: could not read manifest: <details>`.

### 13.3 Undo report

Same structure and ordering as the organize report, with headers
`Folders removed` and `Files restored`, each restore rendered as
`<TypeFolder>/<final name>  ->  <restored name>`, and the totals line
`Totals: N files restored, M folders removed, K conflicts, E errors`.

### 13.4 NFR amendment

NFR-5 is amended for undo: the tool still never deletes user files; undo may
delete only its own manifest file and remove type folders it created that are
now empty.

---

## 14. Iteration 3 — Recursive mode (v0.3.0)

With `--recursive`, files at every depth below the input folder are pulled out
of their subfolders and organized into the **top-level** type folders.

### 14.1 Functional requirements

- **FR-28** — `--recursive` extends the scan to files at any depth below the
  input folder; every file moves into the top-level type folder for its
  extension. Without the flag, behavior is unchanged (top level only).
- **FR-29** — In recursive mode, top-level directories named like type folders
  are destinations, not sources: they are never traversed and appear in
  Skipped with reason `(type folder)`. A name counts as a type folder when it
  is `NO_EXTENSION_Files` or ends in `_Files` with an all-uppercase prefix
  (`TXT_Files`, `TAR-GZ_Files`). Nested directories with such names are
  traversed normally.
- **FR-30** — Symlinks at any depth are skipped and never traversed. Files
  named `.file_organizer_manifest.json` at any depth are skipped with reason
  `(manifest)` so a previously organized subfolder keeps its own undo intact.
- **FR-31** — Files are processed in case-insensitive alphabetical order of
  their relative path, and moves are reported with forward-slash relative
  paths (`sub/notes.txt  ->  TXT_Files/notes.txt`). Collisions between files
  from different source folders get the usual `_N` suffix, first-processed
  file keeps its name.
- **FR-32** — Subfolders emptied by a recursive run are left in place; the
  tool still never deletes user folders.
- **FR-33** — The manifest records relative source paths. `--undo` restores
  nested files to their original folders, recreating intermediate folders
  that have since been deleted, and resolves restore collisions inside the
  original folder. The conflict wording becomes
  `conflict: "<source>" already existed; restored as "<restore path>"`.
- **FR-34** — `--recursive` composes with `--dry-run` (recursive preview, zero
  changes). With `--undo`, `--recursive` is accepted and ignored: the manifest
  fully defines what is restored.

---

## 15. Iteration 4 — Keep structure (v0.4.0)

`--keep-structure` (used with `--recursive`) transports subfolder contents
into the type folders while preserving the source folder organization,
instead of flattening. Three pre-existing subfolders of `.stori` files become
`STORI_Files/batch1/…`, `STORI_Files/batch2/…`, `STORI_Files/batch3/…`.

### 15.1 Functional requirements

- **FR-35** — With `--recursive --keep-structure`, each file's destination
  inside its type folder mirrors the file's source subpath:
  `batch1/a.stori` → `STORI_Files/batch1/a.stori`,
  `a/b/c.stori` → `STORI_Files/a/b/c.stori`. Top-level files land directly in
  the type folder exactly as before. Intermediate destination folders are
  created as needed.
- **FR-36** — `--keep-structure` without `--recursive` exits 2 with
  `Error: --keep-structure requires --recursive`.
- **FR-37** — Subfolders holding more than one file type are split by type,
  with the subpath mirrored in every affected type folder:
  `batch1/a.stori` → `STORI_Files/batch1/a.stori` and
  `batch1/notes.txt` → `TXT_Files/batch1/notes.txt`.
- **FR-38** — Collisions are resolved inside the destination directory
  (`STORI_Files/batch1/`) with the usual lowest-free `_N` suffix and are
  reported as conflicts in Issues.
- **FR-39** — Source folders emptied by the run are removed, deepest first,
  and listed in a `Source folders removed` report section (present only in
  keep-structure runs; the Totals line is unchanged). A folder is removed
  only if it contained at least one moved file (at any depth) and nothing
  remains in it after the run. Pre-existing empty folders are never removed,
  and any leftover (skipped item, failed move, pre-existing empty subfolder)
  keeps the whole chain in place.
- **FR-40** — The manifest records the structured destinations. `--undo`
  restores every file to its original subfolder (recreating source folders
  the run removed), prunes the empty directory shells left inside type
  folders the run created, then removes those type folders. Type folders that
  pre-existed the run are left in place, including any empty shells inside
  them.
- **FR-41** — `--keep-structure` composes with `--dry-run`, including a
  preview of the source-folder removals. With `--undo`, both flags are
  accepted and ignored.

### 15.2 NFR amendment

NFR-5 is further amended: a keep-structure run may remove source folders it
emptied (per FR-39); `--undo` recreates them. User files are still never
deleted.
