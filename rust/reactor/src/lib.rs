//! The reactor: endpoint-independent orchestration.
//!
//! Given a directory snapshot and argv, it decides *everything* — argument
//! validation, classification, collisions, the report text, the manifest JSON,
//! undo — and returns a list of **primitive filesystem operations** plus the
//! stdout/stderr/exit-code. It performs no I/O itself.
//!
//! A *driver* (native Rust or the C harness in the APE) does only two dumb
//! things: snapshot the directory, and replay the ops. All organizing logic is
//! here, so the same reactor is what gherkin-cargo-test verifies (through the
//! native driver) and what ships (through the C driver, compiled via wasm2c).
//!
//! Native drivers call [`run_typed`] directly. The C driver calls the byte
//! entry point [`reactor_run`], which just wraps [`run_typed`] in framing.

use file_organizer_core::{
    build_plan, build_undo_plan, format_report, format_undo_report, Entry, Kind, Manifest,
    MoveError, PlannedMove, RecordedMove, RunResult, UndoResult, MANIFEST_NAME,
};

pub const VERSION: &str = "0.5.0";

// ---------------------------------------------------------------- types ------

/// One primitive filesystem operation the driver replays, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Create this directory and any missing parents (idempotent).
    Mkdir(String),
    /// Rename `from` -> `to` (both relative to the target folder).
    Move { from: String, to: String },
    /// Remove this directory if empty (failure ignored).
    Rmdir(String),
    /// Write these bytes to this path (used for the manifest).
    WriteFile { path: String, content: Vec<u8> },
    /// Delete this file (used to consume the manifest).
    DeleteFile(String),
}

/// Everything a driver observes and hands to the reactor.
pub struct Input {
    pub args: Vec<String>,
    pub exists: bool,
    pub is_dir: bool,
    pub now: String,
    pub folder_display: String,
    pub entries: Vec<Entry>,
    pub manifest: Option<Vec<u8>>,
}

/// Everything the driver must do after the reactor decides.
pub struct Output {
    pub exit: i32,
    pub ops: Vec<Op>,
    pub stdout: String,
    pub stderr: String,
}

enum Action {
    Help,
    Version,
    Bad(String),
    Organize { recursive: bool, keep_structure: bool, dry_run: bool },
    Undo { dry_run: bool },
}

const USAGE: &str =
    "usage: file-organizer [-h] [--dry-run] [--recursive] [--keep-structure] [--undo] [--version] folder";

fn help_text() -> String {
    format!(
        "file-organizer {VERSION}  —  sort a folder's files into type subfolders\n\
A single portable binary: the same file runs on Linux, macOS, Windows, BSD.\n\
\n\
USAGE:\n\
  file-organizer [OPTIONS] <FOLDER>\n\
\n\
OPTIONS:\n\
      --dry-run         preview every action; make NO changes\n\
      --recursive       also pull files out of nested subfolders\n\
      --keep-structure  with --recursive: mirror each file's subpath in its type folder\n\
      --undo            reverse the most recent run (uses the folder's manifest)\n\
  -h, --help            show this help and exit\n\
  -V, --version         print version and exit\n\
\n\
WHAT IT DOES:\n\
  Every top-level file moves into a folder named after its extension, uppercased,\n\
  plus \"_Files\":  notes.txt -> TXT_Files/notes.txt,  Makefile -> NO_EXTENSION_Files/.\n\
  Name clashes get the lowest free numeric suffix (report.txt -> report_1.txt);\n\
  nothing is ever overwritten. Subfolders and symlinks are left alone unless\n\
  --recursive is given.\n\
\n\
  Giving a FOLDER MOVES real files. Try --dry-run first.\n"
    )
}

// ---------------------------------------------------------------- args -------

fn find_folder(args: &[String]) -> Option<&str> {
    args.iter().find(|a| !a.starts_with('-')).map(|s| s.as_str())
}

fn parse_args(args: &[String]) -> Action {
    if args.iter().any(|a| a == "--version" || a == "-V") {
        return Action::Version;
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        return Action::Help;
    }
    let (mut dry, mut recursive, mut keep, mut undo) = (false, false, false, false);
    let mut folder = None;
    for a in args {
        match a.as_str() {
            "--dry-run" => dry = true,
            "--recursive" => recursive = true,
            "--keep-structure" => keep = true,
            "--undo" => undo = true,
            s if s.starts_with('-') => {
                return Action::Bad(format!("{USAGE}\nfile-organizer: error: unrecognized argument: {s}"));
            }
            s => {
                if folder.is_some() {
                    return Action::Bad(format!(
                        "{USAGE}\nfile-organizer: error: unrecognized extra argument: {s}"
                    ));
                }
                folder = Some(s);
            }
        }
    }
    if folder.is_none() {
        return Action::Bad(format!(
            "{USAGE}\nfile-organizer: error: the following arguments are required: folder"
        ));
    }
    if keep && !recursive && !undo {
        return Action::Bad("Error: --keep-structure requires --recursive".to_string());
    }
    if undo {
        Action::Undo { dry_run: dry }
    } else {
        Action::Organize { recursive, keep_structure: keep, dry_run: dry }
    }
}

// ---------------------------------------------------------------- run --------

/// The single decision point. Pure: no I/O, deterministic in its inputs.
pub fn run_typed(inp: Input) -> Output {
    match parse_args(&inp.args) {
        Action::Help => out_text(0, help_text()),
        Action::Version => out_text(0, format!("file-organizer {VERSION}\n")),
        Action::Bad(msg) => Output { exit: 2, ops: vec![], stdout: String::new(), stderr: msg + "\n" },
        Action::Organize { recursive, keep_structure, dry_run } => {
            if let Some(e) = folder_error(&inp) {
                return e;
            }
            organize(&inp, recursive, keep_structure, dry_run)
        }
        Action::Undo { dry_run } => {
            if let Some(e) = folder_error(&inp) {
                return e;
            }
            undo(&inp, dry_run)
        }
    }
}

fn out_text(exit: i32, stdout: String) -> Output {
    Output { exit, ops: vec![], stdout, stderr: String::new() }
}

fn folder_error(inp: &Input) -> Option<Output> {
    let arg = find_folder(&inp.args).unwrap_or("");
    if !inp.exists {
        return Some(err2(format!("Error: path does not exist: {arg}")));
    }
    if !inp.is_dir {
        return Some(err2(format!("Error: path is not a directory: {arg}")));
    }
    None
}

fn err2(msg: String) -> Output {
    Output { exit: 2, ops: vec![], stdout: String::new(), stderr: msg + "\n" }
}

fn parent_of(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

fn organize(inp: &Input, recursive: bool, keep: bool, dry_run: bool) -> Output {
    let plan = build_plan(&inp.entries, recursive, keep);
    let folder = &inp.folder_display;

    if dry_run {
        return out_text(0, format_report(folder, &plan, None, true) + "\n");
    }

    let mut ops = Vec::new();
    // 1. create every destination directory (mkdir -p, deduped, first-seen order)
    let mut seen_dirs: Vec<String> = Vec::new();
    for m in &plan.moves {
        let dest_rel = format!("{}/{}", m.dest_folder, m.final_name);
        let parent = parent_of(&dest_rel).to_string();
        if !parent.is_empty() && !seen_dirs.contains(&parent) {
            seen_dirs.push(parent.clone());
            ops.push(Op::Mkdir(parent));
        }
    }
    // 2. move every file
    for m in &plan.moves {
        ops.push(Op::Move {
            from: m.source.clone(),
            to: format!("{}/{}", m.dest_folder, m.final_name),
        });
    }
    // 3. remove source dirs the run emptied (deepest first)
    if keep {
        let mut dirs = plan.removable_source_dirs.clone();
        dirs.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));
        for rel in dirs {
            ops.push(Op::Rmdir(rel));
        }
    }
    // 4. manifest (only if something moved)
    if !plan.moves.is_empty() {
        let json = manifest_json(&inp.now, &plan.moves, &plan.new_folders);
        ops.push(Op::WriteFile { path: MANIFEST_NAME.to_string(), content: json.into_bytes() });
    }

    let result = RunResult {
        moved: plan.moves.clone(),
        errors: Vec::new(),
        removed_source_dirs: plan.removable_source_dirs.clone(),
    };
    Output {
        exit: 0,
        ops,
        stdout: format_report(folder, &plan, Some(&result), false) + "\n",
        stderr: String::new(),
    }
}

fn undo(inp: &Input, dry_run: bool) -> Output {
    let folder = &inp.folder_display;
    let manifest = match &inp.manifest {
        None => return err2(format!("Error: no manifest found in: {folder}")),
        Some(bytes) => match parse_manifest(bytes) {
            Ok(m) => m,
            Err(e) => return err2(format!("Error: could not read manifest: {e}")),
        },
    };

    let plan = build_undo_plan(&inp.entries, &manifest);
    if dry_run {
        return out_text(0, format_undo_report(folder, &plan, None, true) + "\n");
    }

    let mut ops = Vec::new();
    let mut errors: Vec<MoveError> = Vec::new();
    let mut failed: Vec<RecordedMove> = Vec::new();
    for m in &plan.missing {
        errors.push(MoveError {
            source: format!("{}/{}", m.dest_folder, m.final_name),
            message: "file not found".to_string(),
        });
        failed.push(m.clone());
    }

    // restores: mkdir parents (deduped), then move back
    let mut seen_dirs: Vec<String> = Vec::new();
    for r in &plan.restores {
        let parent = parent_of(&r.restore_name).to_string();
        if !parent.is_empty() && !seen_dirs.contains(&parent) {
            seen_dirs.push(parent.clone());
            ops.push(Op::Mkdir(parent));
        }
    }
    for r in &plan.restores {
        ops.push(Op::Move {
            from: format!("{}/{}", r.dest_folder, r.final_name),
            to: r.restore_name.clone(),
        });
    }
    // remove created type folders: empty inner shells (deepest first) then the folder
    for name in &plan.removable_folders {
        let prefix = format!("{name}/");
        let mut subdirs: Vec<&str> = inp
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Dir && e.rel.starts_with(&prefix))
            .map(|e| e.rel.as_str())
            .collect();
        subdirs.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));
        for s in subdirs {
            ops.push(Op::Rmdir(s.to_string()));
        }
        ops.push(Op::Rmdir(name.clone()));
    }
    // manifest: rewrite the un-restored entries, or delete on full success
    if !failed.is_empty() {
        let kept: Vec<String> = manifest
            .new_folders
            .iter()
            .filter(|f| !plan.removable_folders.contains(f))
            .cloned()
            .collect();
        let recorded: Vec<PlannedMove> = failed
            .iter()
            .map(|m| PlannedMove {
                source: m.source.clone(),
                dest_folder: m.dest_folder.clone(),
                final_name: m.final_name.clone(),
                renamed: false,
            })
            .collect();
        let json = manifest_json(&inp.now, &recorded, &kept);
        ops.push(Op::WriteFile { path: MANIFEST_NAME.to_string(), content: json.into_bytes() });
    } else {
        ops.push(Op::DeleteFile(MANIFEST_NAME.to_string()));
    }

    let result = UndoResult {
        restored: plan.restores.clone(),
        removed_folders: plan.removable_folders.clone(),
        errors,
    };
    let exit = if result.errors.is_empty() { 0 } else { 1 };
    Output {
        exit,
        ops,
        stdout: format_undo_report(folder, &plan, Some(&result), false) + "\n",
        stderr: String::new(),
    }
}

// ---------------------------------------------------------------- manifest ---

fn manifest_json(now: &str, moves: &[PlannedMove], new_folders: &[String]) -> String {
    let recorded: Vec<serde_json::Value> = moves
        .iter()
        .map(|m| {
            serde_json::json!({
                "source": m.source,
                "dest_folder": m.dest_folder,
                "final_name": m.final_name,
            })
        })
        .collect();
    let payload = serde_json::json!({
        "version": 1,
        "created": now,
        "moves": recorded,
        "new_folders": new_folders,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let moves_v = v.get("moves").and_then(|m| m.as_array()).ok_or("'moves'")?;
    let mut moves = Vec::new();
    for m in moves_v {
        let g = |k: &str| m.get(k).and_then(|x| x.as_str()).map(String::from);
        match (g("source"), g("dest_folder"), g("final_name")) {
            (Some(source), Some(dest_folder), Some(final_name)) => {
                moves.push(RecordedMove { source, dest_folder, final_name })
            }
            _ => return Err("malformed move entry".to_string()),
        }
    }
    let new_folders = v
        .get("new_folders")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(Manifest { moves, new_folders })
}

// ---------------------------------------------------------------- framing ----
// A tiny line + length-prefixed protocol shared with the C driver. Paths and
// args are assumed newline-free (documented limitation); blobs are byte-exact.

mod framing;
pub use framing::{parse_input, serialize_output, run};

// ---------------------------------------------------------------- C ABI ------
// For the wasm2c path: the C driver fills a buffer, calls reactor_run, reads the
// framed output back. No imports, no syscalls — same pattern as the bench core.

/// Allocate `len` bytes in linear memory; returns the offset.
///
/// # Safety
/// Caller owns the returned buffer until it is passed to [`reactor_run`].
#[no_mangle]
pub extern "C" fn reactor_alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Run the framed request at `in_ptr`/`in_len`; writes the framed response
/// length through `out_len` and returns its offset.
///
/// # Safety
/// `in_ptr`/`in_len` must describe a buffer from [`reactor_alloc`]; `out_len`
/// must be writable.
#[no_mangle]
pub unsafe extern "C" fn reactor_run(in_ptr: *const u8, in_len: usize, out_len: *mut usize) -> *mut u8 {
    let input = std::slice::from_raw_parts(in_ptr, in_len);
    let out = run(input);
    *out_len = out.len();
    let mut boxed = out.into_boxed_slice();
    let p = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    p
}
