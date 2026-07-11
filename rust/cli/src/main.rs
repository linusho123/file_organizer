//! Native command-line front-end: argument parsing, real filesystem I/O,
//! manifest persistence, and exit codes. All planning/formatting is delegated
//! to `file_organizer_core`, which never touches the filesystem.

use std::path::Path;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

use file_organizer_core as core;
use file_organizer_core::{
    build_plan, build_undo_plan, format_report, format_undo_report, Entry, Kind, Manifest,
    MoveError, Plan, RecordedMove, RunResult, UndoPlan, UndoResult, MANIFEST_NAME,
};
use serde_json::{json, Value};

const VERSION: &str = "0.5.0";
const USAGE: &str = "usage: file-organizer [-h] [--dry-run] [--recursive] [--keep-structure] [--undo] [--version] folder";

fn main() {
    exit(run(std::env::args().skip(1).collect()));
}

struct Args {
    folder: Option<String>,
    dry_run: bool,
    recursive: bool,
    keep_structure: bool,
    undo: bool,
}

fn run(argv: Vec<String>) -> i32 {
    // Eager flags, matching argparse: --version / --help short-circuit.
    if argv.iter().any(|a| a == "--version") {
        println!("file-organizer {VERSION}");
        return 0;
    }
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return 0;
    }

    let mut args = Args {
        folder: None,
        dry_run: false,
        recursive: false,
        keep_structure: false,
        undo: false,
    };
    for a in &argv {
        match a.as_str() {
            "--dry-run" => args.dry_run = true,
            "--recursive" => args.recursive = true,
            "--keep-structure" => args.keep_structure = true,
            "--undo" => args.undo = true,
            s if s.starts_with('-') => {
                eprintln!("{USAGE}\nfile-organizer: error: unrecognized argument: {s}");
                return 2;
            }
            s => {
                if args.folder.is_some() {
                    eprintln!("{USAGE}\nfile-organizer: error: unrecognized extra argument: {s}");
                    return 2;
                }
                args.folder = Some(s.to_string());
            }
        }
    }

    let folder = match &args.folder {
        Some(f) => f.clone(),
        None => {
            eprintln!("{USAGE}\nfile-organizer: error: the following arguments are required: folder");
            return 2;
        }
    };

    if args.keep_structure && !args.recursive && !args.undo {
        eprintln!("Error: --keep-structure requires --recursive");
        return 2;
    }

    let target = Path::new(&folder);
    if !target.exists() {
        eprintln!("Error: path does not exist: {folder}");
        return 2;
    }
    if !target.is_dir() {
        eprintln!("Error: path is not a directory: {folder}");
        return 2;
    }

    let folder_abs = std::fs::canonicalize(target).unwrap_or_else(|_| target.to_path_buf());
    let folder_str = folder_abs.display().to_string();

    if args.undo {
        return run_undo(&folder_abs, &folder_str, args.dry_run);
    }

    let entries = snapshot(&folder_abs);
    let plan = build_plan(&entries, args.recursive, args.keep_structure);
    if args.dry_run {
        println!("{}", format_report(&folder_str, &plan, None, true));
        return 0;
    }
    let result = execute_plan(&folder_abs, &plan);
    println!("{}", format_report(&folder_str, &plan, Some(&result), false));
    write_manifest(&folder_abs, &plan, &result);
    if result.errors.is_empty() {
        0
    } else {
        1
    }
}

fn run_undo(folder: &Path, folder_str: &str, dry_run: bool) -> i32 {
    let manifest = match read_manifest(folder) {
        Err(e) => {
            eprintln!("Error: could not read manifest: {e}");
            return 2;
        }
        Ok(None) => {
            eprintln!("Error: no manifest found in: {folder_str}");
            return 2;
        }
        Ok(Some(m)) => m,
    };
    let entries = snapshot(folder);
    let plan = build_undo_plan(&entries, &manifest);
    if dry_run {
        println!("{}", format_undo_report(folder_str, &plan, None, true));
        return 0;
    }
    let result = execute_undo(folder, &manifest, &plan);
    println!("{}", format_undo_report(folder_str, &plan, Some(&result), false));
    if result.errors.is_empty() {
        0
    } else {
        1
    }
}

// --- filesystem snapshot ----------------------------------------------------

/// Recursively walk `root`, producing a snapshot the pure core can plan over.
/// Symlinks are recorded but never followed (lstat semantics).
fn snapshot(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Entry>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let kind = match std::fs::symlink_metadata(&path) {
            Ok(m) => {
                let ft = m.file_type();
                if ft.is_symlink() {
                    Kind::Symlink
                } else if ft.is_dir() {
                    Kind::Dir
                } else if ft.is_file() {
                    Kind::File
                } else {
                    Kind::Other
                }
            }
            Err(_) => Kind::Other,
        };
        let descend = kind == Kind::Dir;
        out.push(Entry { rel, kind });
        if descend {
            walk(root, &path, out);
        }
    }
}

// --- execution --------------------------------------------------------------

fn move_file(src: &Path, dst: &Path) -> Result<(), String> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) => match std::fs::copy(src, dst) {
            Ok(_) => std::fs::remove_file(src).map_err(|e| e.to_string()),
            Err(_) => Err(e.to_string()),
        },
    }
}

fn execute_plan(folder: &Path, plan: &Plan) -> RunResult {
    let mut result = RunResult::default();
    for m in &plan.moves {
        let src = folder.join(&m.source);
        let dst = folder.join(&m.dest_folder).join(&m.final_name);
        if let Some(p) = dst.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        match move_file(&src, &dst) {
            Ok(()) => result.moved.push(m.clone()),
            Err(e) => result.errors.push(MoveError {
                source: m.source.clone(),
                message: e,
            }),
        }
    }
    let mut dirs = plan.removable_source_dirs.clone();
    dirs.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));
    for rel in dirs {
        if std::fs::remove_dir(folder.join(&rel)).is_ok() {
            result.removed_source_dirs.push(rel);
        }
    }
    result
}

fn execute_undo(folder: &Path, manifest: &Manifest, plan: &UndoPlan) -> UndoResult {
    let mut result = UndoResult::default();
    let mut failed: Vec<RecordedMove> = Vec::new();

    for m in &plan.missing {
        result.errors.push(MoveError {
            source: format!("{}/{}", m.dest_folder, m.final_name),
            message: "file not found".to_string(),
        });
        failed.push(m.clone());
    }
    for r in &plan.restores {
        let src = folder.join(&r.dest_folder).join(&r.final_name);
        let dst = folder.join(&r.restore_name);
        if let Some(p) = dst.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        match move_file(&src, &dst) {
            Ok(()) => result.restored.push(r.clone()),
            Err(e) => {
                result.errors.push(MoveError {
                    source: format!("{}/{}", r.dest_folder, r.final_name),
                    message: e,
                });
                failed.push(RecordedMove {
                    source: r.source.clone(),
                    dest_folder: r.dest_folder.clone(),
                    final_name: r.final_name.clone(),
                });
            }
        }
    }
    for name in &plan.removable_folders {
        let path = folder.join(name);
        prune_empty_dirs(&path);
        if std::fs::remove_dir(&path).is_ok() {
            result.removed_folders.push(name.clone());
        }
    }

    let manifest_path = folder.join(MANIFEST_NAME);
    if !failed.is_empty() {
        let kept: Vec<String> = manifest
            .new_folders
            .iter()
            .filter(|f| !result.removed_folders.contains(f))
            .cloned()
            .collect();
        write_manifest_payload(folder, &failed, &kept);
    } else if manifest_path.is_file() {
        let _ = std::fs::remove_file(&manifest_path);
    }
    result
}

fn prune_empty_dirs(root: &Path) {
    let rd = match std::fs::read_dir(root) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if let Ok(m) = std::fs::symlink_metadata(&path) {
            if m.file_type().is_dir() {
                prune_empty_dirs(&path);
                let _ = std::fs::remove_dir(&path);
            }
        }
    }
}

// --- manifest persistence ---------------------------------------------------

fn write_manifest(folder: &Path, plan: &Plan, result: &RunResult) {
    if result.moved.is_empty() {
        return;
    }
    write_manifest_payload_from_moves(folder, &result.moved, &plan.new_folders);
}

fn write_manifest_payload_from_moves(folder: &Path, moves: &[core::PlannedMove], new_folders: &[String]) {
    let recorded: Vec<Value> = moves
        .iter()
        .map(|m| json!({"source": m.source, "dest_folder": m.dest_folder, "final_name": m.final_name}))
        .collect();
    write_json(folder, recorded, new_folders);
}

fn write_manifest_payload(folder: &Path, moves: &[RecordedMove], new_folders: &[String]) {
    let recorded: Vec<Value> = moves
        .iter()
        .map(|m| json!({"source": m.source, "dest_folder": m.dest_folder, "final_name": m.final_name}))
        .collect();
    write_json(folder, recorded, new_folders);
}

fn write_json(folder: &Path, moves: Vec<Value>, new_folders: &[String]) {
    let payload = json!({
        "version": 1,
        "created": iso8601_now(),
        "moves": moves,
        "new_folders": new_folders,
    });
    let _ = std::fs::write(
        folder.join(MANIFEST_NAME),
        serde_json::to_string_pretty(&payload).unwrap(),
    );
}

fn read_manifest(folder: &Path) -> Result<Option<Manifest>, String> {
    let path = folder.join(MANIFEST_NAME);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let moves_v = v
        .get("moves")
        .and_then(|m| m.as_array())
        .ok_or_else(|| "'moves'".to_string())?;
    let mut moves = Vec::new();
    for m in moves_v {
        let g = |k: &str| m.get(k).and_then(|x| x.as_str()).map(String::from);
        let (source, dest_folder, final_name) = (g("source"), g("dest_folder"), g("final_name"));
        match (source, dest_folder, final_name) {
            (Some(source), Some(dest_folder), Some(final_name)) => moves.push(RecordedMove {
                source,
                dest_folder,
                final_name,
            }),
            _ => return Err("malformed move entry".to_string()),
        }
    }
    let new_folders = v
        .get("new_folders")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
        .unwrap_or_default();
    Ok(Some(Manifest { moves, new_folders }))
}

fn iso8601_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}+00:00")
}

/// Howard Hinnant's days-from-civil inverse; UTC calendar date from Unix days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}
