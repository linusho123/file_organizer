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

pub const VERSION: &str = "0.6.0";

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
    Organize { recursive: bool, keep_structure: bool, move_folders: bool, dry_run: bool },
    Undo { dry_run: bool },
}

const USAGE: &str =
    "usage: file-organizer [-h] [--dry-run] [--recursive] [--keep-structure] [--move-folders] [--undo] [--version] folder";

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
      --move-folders    with --recursive --keep-structure: transport single-type\n\
                        subfolders whole (one rename) instead of file by file\n\
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
    let (mut dry, mut recursive, mut keep, mut move_folders, mut undo) =
        (false, false, false, false, false);
    let mut folder = None;
    for a in args {
        match a.as_str() {
            "--dry-run" => dry = true,
            "--recursive" => recursive = true,
            "--keep-structure" => keep = true,
            "--move-folders" => move_folders = true,
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
    if move_folders && !(recursive && keep) && !undo {
        return Action::Bad(
            "Error: --move-folders requires --recursive --keep-structure".to_string(),
        );
    }
    if undo {
        Action::Undo { dry_run: dry }
    } else {
        Action::Organize { recursive, keep_structure: keep, move_folders, dry_run: dry }
    }
}

// ---------------------------------------------------------------- run --------

/// The single decision point. Pure: no I/O, deterministic in its inputs.
pub fn run_typed(inp: Input) -> Output {
    match parse_args(&inp.args) {
        Action::Help => out_text(0, help_text()),
        Action::Version => out_text(0, format!("file-organizer {VERSION}\n")),
        Action::Bad(msg) => Output { exit: 2, ops: vec![], stdout: String::new(), stderr: msg + "\n" },
        Action::Organize { recursive, keep_structure, move_folders, dry_run } => {
            if let Some(e) = folder_error(&inp) {
                return e;
            }
            organize(&inp, recursive, keep_structure, move_folders, dry_run)
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

fn organize(inp: &Input, recursive: bool, keep: bool, move_folders: bool, dry_run: bool) -> Output {
    let plan = build_plan(&inp.entries, recursive, keep, move_folders);
    let folder = &inp.folder_display;

    if dry_run {
        return out_text(0, format_report(folder, &plan, None, true) + "\n");
    }

    let mut ops = Vec::new();
    let mut seen_dirs: Vec<String> = Vec::new();
    // 1. transport whole folders: mkdir the type folder, then one rename each
    for m in &plan.folder_moves {
        if !seen_dirs.contains(&m.dest_folder) {
            seen_dirs.push(m.dest_folder.clone());
            ops.push(Op::Mkdir(m.dest_folder.clone()));
        }
        ops.push(Op::Move {
            from: m.source.clone(),
            to: format!("{}/{}", m.dest_folder, m.final_name),
        });
    }
    // 2. create every per-file destination directory (mkdir -p, deduped, first-seen order)
    for m in &plan.moves {
        let dest_rel = format!("{}/{}", m.dest_folder, m.final_name);
        let parent = parent_of(&dest_rel).to_string();
        if !parent.is_empty() && !seen_dirs.contains(&parent) {
            seen_dirs.push(parent.clone());
            ops.push(Op::Mkdir(parent));
        }
    }
    // 3. move every file
    for m in &plan.moves {
        ops.push(Op::Move {
            from: m.source.clone(),
            to: format!("{}/{}", m.dest_folder, m.final_name),
        });
    }
    // 4. remove source dirs the run emptied (deepest first)
    if keep {
        let mut dirs = plan.removable_source_dirs.clone();
        dirs.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));
        for rel in dirs {
            ops.push(Op::Rmdir(rel));
        }
    }
    // 5. manifest (only if something moved)
    if !plan.moves.is_empty() || !plan.folder_moves.is_empty() {
        let folder_moves: Vec<RecordedMove> = plan
            .folder_moves
            .iter()
            .map(|m| RecordedMove {
                source: m.source.clone(),
                dest_folder: m.dest_folder.clone(),
                final_name: m.final_name.clone(),
            })
            .collect();
        let json = manifest_json(&inp.now, &plan.moves, &folder_moves, &plan.new_folders);
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
    let mut failed_folders: Vec<RecordedMove> = Vec::new();
    for m in &plan.missing {
        errors.push(MoveError {
            source: format!("{}/{}", m.dest_folder, m.final_name),
            message: "file not found".to_string(),
        });
        failed.push(m.clone());
    }
    for m in &plan.missing_folders {
        errors.push(MoveError {
            source: format!("{}/{}", m.dest_folder, m.final_name),
            message: "folder not found".to_string(),
        });
        failed_folders.push(m.clone());
    }

    // transported folders first: one rename each back to the top level
    for r in &plan.folder_restores {
        ops.push(Op::Move {
            from: format!("{}/{}", r.dest_folder, r.final_name),
            to: r.restore_name.clone(),
        });
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
    if !failed.is_empty() || !failed_folders.is_empty() {
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
        let json = manifest_json(&inp.now, &recorded, &failed_folders, &kept);
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

fn manifest_json(
    now: &str,
    moves: &[PlannedMove],
    folder_moves: &[RecordedMove],
    new_folders: &[String],
) -> String {
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
    let mut payload = serde_json::json!({
        "version": 1,
        "created": now,
        "moves": recorded,
        "new_folders": new_folders,
    });
    // Only move-folders runs record this key; ordinary manifests stay
    // byte-identical to the pre-0.6 (and Python) format.
    if !folder_moves.is_empty() {
        let recorded_folders: Vec<serde_json::Value> = folder_moves
            .iter()
            .map(|m| {
                serde_json::json!({
                    "source": m.source,
                    "dest_folder": m.dest_folder,
                    "final_name": m.final_name,
                })
            })
            .collect();
        payload["folder_moves"] = serde_json::Value::Array(recorded_folders);
    }
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let parse_moves = |arr: &[serde_json::Value]| -> Result<Vec<RecordedMove>, String> {
        let mut moves = Vec::new();
        for m in arr {
            let g = |k: &str| m.get(k).and_then(|x| x.as_str()).map(String::from);
            match (g("source"), g("dest_folder"), g("final_name")) {
                (Some(source), Some(dest_folder), Some(final_name)) => {
                    moves.push(RecordedMove { source, dest_folder, final_name })
                }
                _ => return Err("malformed move entry".to_string()),
            }
        }
        Ok(moves)
    };
    let moves_v = v.get("moves").and_then(|m| m.as_array()).ok_or("'moves'")?;
    let moves = parse_moves(moves_v)?;
    let folder_moves = match v.get("folder_moves").and_then(|m| m.as_array()) {
        Some(arr) => parse_moves(arr)?,
        None => Vec::new(),
    };
    let new_folders = v
        .get("new_folders")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(Manifest { moves, folder_moves, new_folders })
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

// ---------------------------------------------------------------- tests ------

#[cfg(test)]
mod tests {
    use super::*;

    fn input(args: &[&str], entries: Vec<Entry>, manifest: Option<Vec<u8>>) -> Input {
        Input {
            args: args.iter().map(|s| s.to_string()).collect(),
            exists: true,
            is_dir: true,
            now: "2026-01-01T00:00:00+00:00".to_string(),
            folder_display: "/x".to_string(),
            entries,
            manifest,
        }
    }

    fn f(rel: &str) -> Entry {
        Entry::new(rel, Kind::File)
    }
    fn d(rel: &str) -> Entry {
        Entry::new(rel, Kind::Dir)
    }

    #[test]
    fn move_folders_alone_is_rejected() {
        let out = run_typed(input(&["--move-folders", "target"], vec![], None));
        assert_eq!(out.exit, 2);
        assert!(out.stderr.contains("Error: --move-folders requires --recursive --keep-structure"));
        assert!(out.ops.is_empty());
    }

    #[test]
    fn move_folders_with_only_recursive_is_rejected() {
        let out = run_typed(input(&["--recursive", "--move-folders", "target"], vec![], None));
        assert_eq!(out.exit, 2);
        assert!(out.stderr.contains("Error: --move-folders requires --recursive --keep-structure"));
    }

    #[test]
    fn move_folders_is_accepted_and_ignored_with_undo() {
        let out = run_typed(input(&["--undo", "--move-folders", "target"], vec![], None));
        // No manifest in the input: undo's own error path, not an argument error.
        assert_eq!(out.exit, 2);
        assert!(out.stderr.contains("no manifest found"));
    }

    #[test]
    fn transport_emits_one_mkdir_one_move_and_a_folder_manifest() {
        let entries = vec![d("batch1"), f("batch1/a.stori")];
        let out = run_typed(input(
            &["--recursive", "--keep-structure", "--move-folders", "target"],
            entries,
            None,
        ));
        assert_eq!(out.exit, 0);
        assert_eq!(out.ops.len(), 3, "mkdir + move + manifest, got {:?}", out.ops);
        assert_eq!(out.ops[0], Op::Mkdir("STORI_Files".to_string()));
        assert_eq!(
            out.ops[1],
            Op::Move { from: "batch1".to_string(), to: "STORI_Files/batch1".to_string() }
        );
        match &out.ops[2] {
            Op::WriteFile { path, content } => {
                assert_eq!(path, MANIFEST_NAME);
                let json = String::from_utf8_lossy(content);
                assert!(json.contains("\"folder_moves\""));
            }
            other => panic!("expected manifest write, got {other:?}"),
        }
    }

    #[test]
    fn dry_run_transport_emits_no_ops() {
        let entries = vec![d("batch1"), f("batch1/a.stori")];
        let out = run_typed(input(
            &["--recursive", "--keep-structure", "--move-folders", "--dry-run", "target"],
            entries,
            None,
        ));
        assert_eq!(out.exit, 0);
        assert!(out.ops.is_empty());
        assert!(out.stdout.contains("DRY RUN - no changes made"));
        assert!(out.stdout.contains("batch1/  ->  STORI_Files/batch1/  (1 file)"));
    }

    #[test]
    fn manifest_round_trips_through_undo() {
        // Organize, capture the manifest the reactor writes...
        let out = run_typed(input(
            &["--recursive", "--keep-structure", "--move-folders", "target"],
            vec![d("batch1"), f("batch1/a.stori")],
            None,
        ));
        let manifest = out
            .ops
            .iter()
            .find_map(|op| match op {
                Op::WriteFile { content, .. } => Some(content.clone()),
                _ => None,
            })
            .expect("organize wrote a manifest");

        // ...then undo against the post-run snapshot.
        let entries = vec![d("STORI_Files"), d("STORI_Files/batch1"), f("STORI_Files/batch1/a.stori")];
        let out = run_typed(input(&["--undo", "target"], entries, Some(manifest)));
        assert_eq!(out.exit, 0);
        assert!(out.stdout.contains("Folders restored:"));
        assert!(out.stdout.contains("STORI_Files/batch1/  ->  batch1/"));
        assert_eq!(
            out.ops[0],
            Op::Move { from: "STORI_Files/batch1".to_string(), to: "batch1".to_string() }
        );
        assert!(out.ops.contains(&Op::Rmdir("STORI_Files".to_string())));
        assert_eq!(*out.ops.last().unwrap(), Op::DeleteFile(MANIFEST_NAME.to_string()));
    }

    #[test]
    fn undo_with_a_missing_folder_rewrites_the_manifest_and_exits_1() {
        let manifest = br#"{
            "version": 1,
            "created": "2026-01-01T00:00:00+00:00",
            "moves": [],
            "folder_moves": [
                {"source": "batch1", "dest_folder": "STORI_Files", "final_name": "batch1"}
            ],
            "new_folders": ["STORI_Files"]
        }"#;
        let out = run_typed(input(&["--undo", "target"], vec![d("STORI_Files")], Some(manifest.to_vec())));
        assert_eq!(out.exit, 1);
        assert!(out.stdout.contains("error: could not restore \"STORI_Files/batch1\": folder not found"));
        let rewrote = out.ops.iter().any(|op| matches!(
            op,
            Op::WriteFile { path, content }
                if path == MANIFEST_NAME
                    && String::from_utf8_lossy(content).contains("\"folder_moves\"")
        ));
        assert!(rewrote, "manifest should be rewritten with the failed folder, ops: {:?}", out.ops);
    }

    #[test]
    fn plain_manifests_have_no_folder_moves_key() {
        let out = run_typed(input(&["target"], vec![f("notes.txt")], None));
        let manifest = out
            .ops
            .iter()
            .find_map(|op| match op {
                Op::WriteFile { content, .. } => Some(String::from_utf8_lossy(content).into_owned()),
                _ => None,
            })
            .expect("organize wrote a manifest");
        assert!(!manifest.contains("folder_moves"));
    }
}
