//! Pure planning, undo-planning, and report formatting for the file organizer.
//!
//! This crate contains **no filesystem access**. All input arrives as a
//! snapshot of directory entries ([`Entry`]); all output is a plan or a
//! formatted report string. That keeps the logic endpoint-independent: the
//! same code compiles to native, wasm32-wasip1, and wasm32-unknown-unknown
//! (for wasm2c). The native CLI wraps it with real I/O.
//!
//! Behaviour is a faithful port of the Python `file_organizer` package; report
//! strings are byte-for-byte identical so the same Gherkin scenarios pass.

use std::collections::{HashMap, HashSet};

pub const NO_EXTENSION_FOLDER: &str = "NO_EXTENSION_Files";
pub const MANIFEST_NAME: &str = ".file_organizer_manifest.json";
const TYPE_FOLDER_SUFFIX: &str = "_Files";

/// The kind of a directory entry, as seen by an `lstat`-style probe
/// (symlinks are never followed, matching Python's `is_symlink()`-first order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
    Symlink,
    Other,
}

/// One entry in the recursive snapshot of the target folder. `rel` is a
/// forward-slash relative path (no leading slash), at any depth.
#[derive(Clone, Debug)]
pub struct Entry {
    pub rel: String,
    pub kind: Kind,
}

impl Entry {
    pub fn new(rel: impl Into<String>, kind: Kind) -> Self {
        Entry { rel: rel.into(), kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedMove {
    pub source: String,
    pub dest_folder: String,
    pub final_name: String,
    pub renamed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedItem {
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub moves: Vec<PlannedMove>,
    pub new_folders: Vec<String>,
    pub skipped: Vec<SkippedItem>,
    pub keep_structure: bool,
    pub removable_source_dirs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveError {
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct RunResult {
    pub moved: Vec<PlannedMove>,
    pub errors: Vec<MoveError>,
    pub removed_source_dirs: Vec<String>,
}

// --- classification primitives ----------------------------------------------

/// Lowercased extension of `name` (substring after the last dot), or `None`.
/// A leading dot (dotfiles) or a trailing dot does not count.
pub fn get_extension(name: &str) -> Option<String> {
    match name.rfind('.') {
        Some(i) if i > 0 => {
            let ext = &name[i + 1..];
            if ext.is_empty() {
                None
            } else {
                Some(ext.to_lowercase())
            }
        }
        _ => None,
    }
}

/// Type-folder name for an extension (`txt` -> `TXT_Files`).
pub fn folder_name_for(ext: Option<&str>) -> String {
    match ext {
        None => NO_EXTENSION_FOLDER.to_string(),
        Some(e) => format!("{}{}", e.to_uppercase(), TYPE_FOLDER_SUFFIX),
    }
}

/// True if `name` looks like a folder this tool creates as a destination.
pub fn is_type_folder(name: &str) -> bool {
    if name == NO_EXTENSION_FOLDER {
        return true;
    }
    if !name.ends_with(TYPE_FOLDER_SUFFIX) {
        return false;
    }
    let prefix = &name[..name.len() - TYPE_FOLDER_SUFFIX.len()];
    !prefix.is_empty() && prefix == prefix.to_uppercase().as_str()
}

/// Return a destination filename avoiding `taken` (a set of lowercased names).
/// The lowest free `_N` suffix is inserted before the extension.
pub fn resolve_name(name: &str, taken: &HashSet<String>) -> (String, bool) {
    if !taken.contains(&name.to_lowercase()) {
        return (name.to_string(), false);
    }
    let (stem, suffix) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let mut counter = 1u64;
    loop {
        let candidate = format!("{stem}_{counter}{suffix}");
        if !taken.contains(&candidate.to_lowercase()) {
            return (candidate, true);
        }
        counter += 1;
    }
}

// --- snapshot indexing ------------------------------------------------------

fn split_parent(rel: &str) -> (&str, &str) {
    match rel.rfind('/') {
        Some(i) => (&rel[..i], &rel[i + 1..]),
        None => ("", rel),
    }
}

/// Indexed view over a snapshot: children-by-parent (sorted case-insensitively
/// by basename, matching Python's per-directory `sorted(..., key=name.lower)`),
/// and the set of directory paths.
struct Index<'a> {
    entries: &'a [Entry],
    by_parent: HashMap<&'a str, Vec<usize>>,
    dirs: HashSet<&'a str>,
}

impl<'a> Index<'a> {
    fn new(entries: &'a [Entry]) -> Self {
        let mut by_parent: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut dirs: HashSet<&str> = HashSet::new();
        for (i, e) in entries.iter().enumerate() {
            let (parent, _) = split_parent(&e.rel);
            by_parent.entry(parent).or_default().push(i);
            if e.kind == Kind::Dir {
                dirs.insert(e.rel.as_str());
            }
        }
        for idxs in by_parent.values_mut() {
            idxs.sort_by(|&a, &b| {
                let na = split_parent(&entries[a].rel).1.to_lowercase();
                let nb = split_parent(&entries[b].rel).1.to_lowercase();
                na.cmp(&nb)
            });
        }
        Index { entries, by_parent, dirs }
    }

    fn children(&self, dir: &str) -> &[usize] {
        self.by_parent.get(dir).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn is_dir(&self, path: &str) -> bool {
        self.dirs.contains(path)
    }

    /// Lowercased basenames of the immediate children of `dir` (matches
    /// `{p.name.lower() for p in dir.iterdir()}`).
    fn child_names_lower(&self, dir: &str) -> HashSet<String> {
        self.children(dir)
            .iter()
            .map(|&i| split_parent(&self.entries[i].rel).1.to_lowercase())
            .collect()
    }
}

// --- planning ---------------------------------------------------------------

struct ScanOut {
    files: Vec<String>,
    skipped: Vec<SkippedItem>,
    source_dirs: Vec<String>,
}

fn scan(idx: &Index, dir: &str, top: bool, recursive: bool, out: &mut ScanOut) {
    for &i in idx.children(dir) {
        let e = &idx.entries[i];
        let rel = e.rel.clone();
        let name = split_parent(&rel).1;
        if name == MANIFEST_NAME {
            out.skipped.push(SkippedItem { name: rel, reason: "manifest".into() });
            continue;
        }
        if e.kind == Kind::Symlink {
            out.skipped.push(SkippedItem { name: rel, reason: "symlink".into() });
            continue;
        }
        if e.kind == Kind::Dir {
            if !recursive {
                out.skipped.push(SkippedItem { name: rel, reason: "directory".into() });
            } else if top && is_type_folder(name) {
                out.skipped.push(SkippedItem { name: rel, reason: "type folder".into() });
            } else {
                out.source_dirs.push(rel.clone());
                scan(idx, &rel, false, recursive, out);
            }
            continue;
        }
        if e.kind != Kind::File {
            out.skipped
                .push(SkippedItem { name: rel, reason: "not a regular file".into() });
            continue;
        }
        out.files.push(rel);
    }
}

/// Scan `entries` and plan every move without any filesystem changes — the
/// pure port of Python's `build_plan`.
pub fn build_plan(entries: &[Entry], recursive: bool, keep_structure: bool) -> Plan {
    let idx = Index::new(entries);
    let mut plan = Plan { keep_structure, ..Default::default() };
    let mut out = ScanOut { files: Vec::new(), skipped: Vec::new(), source_dirs: Vec::new() };

    scan(&idx, "", true, recursive, &mut out);
    plan.skipped = out.skipped;
    out.files.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

    let mut seen_dest: HashSet<String> = HashSet::new();
    let mut taken: HashMap<(String, String), HashSet<String>> = HashMap::new();

    for rel in &out.files {
        let (parent, basename) = split_parent(rel);
        let ext = get_extension(basename);
        let dest_folder = folder_name_for(ext.as_deref());
        if !seen_dest.contains(&dest_folder) {
            seen_dest.insert(dest_folder.clone());
            if !idx.is_dir(&dest_folder) {
                plan.new_folders.push(dest_folder.clone());
            }
        }
        let dest_parent = if keep_structure { parent } else { "" };
        let key = (dest_folder.clone(), dest_parent.to_string());
        if !taken.contains_key(&key) {
            let dest_dir = if dest_parent.is_empty() {
                dest_folder.clone()
            } else {
                format!("{dest_folder}/{dest_parent}")
            };
            let set = if idx.is_dir(&dest_dir) {
                idx.child_names_lower(&dest_dir)
            } else {
                HashSet::new()
            };
            taken.insert(key.clone(), set);
        }
        let set = taken.get_mut(&key).unwrap();
        let (final_base, renamed) = resolve_name(basename, set);
        set.insert(final_base.to_lowercase());
        let final_name = if dest_parent.is_empty() {
            final_base.clone()
        } else {
            format!("{dest_parent}/{final_base}")
        };
        plan.moves.push(PlannedMove {
            source: rel.clone(),
            dest_folder,
            final_name,
            renamed,
        });
    }

    if keep_structure {
        plan.removable_source_dirs = plan_removals(&idx, &out.source_dirs, &plan.moves);
    }
    plan
}

/// Source dirs the run empties: at least one moved file under them, nothing
/// left over. Deepest-first so a parent can see its children already removable.
fn plan_removals(idx: &Index, source_dirs: &[String], moves: &[PlannedMove]) -> Vec<String> {
    let moved: HashSet<&str> = moves.iter().map(|m| m.source.as_str()).collect();
    let mut removable: HashSet<String> = HashSet::new();

    let mut order: Vec<&String> = source_dirs.iter().collect();
    // reverse=True on depth; Rust's sort_by is stable, matching Python's sorted.
    order.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()));

    for rel in order {
        let prefix = format!("{rel}/");
        if !moved.iter().any(|m| m.starts_with(&prefix)) {
            continue;
        }
        let mut empties = true;
        for &ci in idx.children(rel) {
            let child = &idx.entries[ci];
            let child_rel = child.rel.as_str();
            if child.kind == Kind::File && moved.contains(child_rel) {
                continue;
            }
            if child.kind == Kind::Dir && removable.contains(child_rel) {
                continue;
            }
            empties = false;
            break;
        }
        if empties {
            removable.insert(rel.clone());
        }
    }

    let mut result: Vec<String> = removable.into_iter().collect();
    result.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    result
}

// --- undo planning ----------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedMove {
    pub source: String,
    pub dest_folder: String,
    pub final_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    pub moves: Vec<RecordedMove>,
    pub new_folders: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedRestore {
    pub source: String,
    pub dest_folder: String,
    pub final_name: String,
    pub restore_name: String,
    pub renamed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UndoPlan {
    pub restores: Vec<PlannedRestore>,
    pub missing: Vec<RecordedMove>,
    pub removable_folders: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct UndoResult {
    pub restored: Vec<PlannedRestore>,
    pub removed_folders: Vec<String>,
    pub errors: Vec<MoveError>,
}

/// Plan every restore and folder removal from a manifest + snapshot — the pure
/// port of Python's `build_undo_plan`.
pub fn build_undo_plan(entries: &[Entry], manifest: &Manifest) -> UndoPlan {
    let idx = Index::new(entries);
    let mut plan = UndoPlan::default();
    let mut taken_by_dir: HashMap<String, HashSet<String>> = HashMap::new();

    for m in &manifest.moves {
        let current = format!("{}/{}", m.dest_folder, m.final_name);
        if !is_file(&idx, &current) {
            plan.missing.push(m.clone());
            continue;
        }
        let (parent, base) = split_parent(&m.source);
        if !taken_by_dir.contains_key(parent) {
            let set = if parent.is_empty() || idx.is_dir(parent) {
                idx.child_names_lower(parent)
            } else {
                HashSet::new()
            };
            taken_by_dir.insert(parent.to_string(), set);
        }
        let set = taken_by_dir.get_mut(parent).unwrap();
        let (restore_base, renamed) = resolve_name(base, set);
        set.insert(restore_base.to_lowercase());
        let restore_name = if parent.is_empty() {
            restore_base.clone()
        } else {
            format!("{parent}/{restore_base}")
        };
        plan.restores.push(PlannedRestore {
            source: m.source.clone(),
            dest_folder: m.dest_folder.clone(),
            final_name: m.final_name.clone(),
            restore_name,
            renamed,
        });
    }

    let restored_away: HashSet<(String, String)> = plan
        .restores
        .iter()
        .map(|r| (r.dest_folder.to_lowercase(), r.final_name.to_lowercase()))
        .collect();

    for name in &manifest.new_folders {
        if !idx.is_dir(name) {
            continue;
        }
        let prefix = format!("{name}/");
        let leftover = idx.entries.iter().any(|e| {
            if !e.rel.starts_with(&prefix) || e.kind == Kind::Dir {
                return false;
            }
            let within = e.rel[prefix.len()..].to_lowercase();
            !restored_away.contains(&(name.to_lowercase(), within))
        });
        if !leftover {
            plan.removable_folders.push(name.clone());
        }
    }
    plan
}

fn is_file(idx: &Index, path: &str) -> bool {
    idx.entries
        .iter()
        .any(|e| e.rel == path && e.kind == Kind::File)
}

// --- report formatting (byte-identical to Python's report.py) ---------------

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

fn section(lines: &mut Vec<String>, title: &str, items: &[String]) {
    lines.push(format!("{title}:"));
    if items.is_empty() {
        lines.push("  none".to_string());
    } else {
        for item in items {
            lines.push(format!("  {item}"));
        }
    }
    lines.push(String::new());
}

/// Render the organize summary report. `result` is `None` for dry runs.
pub fn format_report(folder: &str, plan: &Plan, result: Option<&RunResult>, dry_run: bool) -> String {
    let moves: &[PlannedMove] = match result {
        None => &plan.moves,
        Some(r) => &r.moved,
    };
    let empty_errors: Vec<MoveError> = Vec::new();
    let errors: &[MoveError] = match result {
        None => &empty_errors,
        Some(r) => &r.errors,
    };

    let mut lines: Vec<String> = Vec::new();
    if dry_run {
        lines.push("DRY RUN - no changes made".to_string());
    }
    lines.push(format!("Organizing: {folder}"));
    lines.push(String::new());

    section(&mut lines, "Folders created", &plan.new_folders);
    let move_lines: Vec<String> = moves
        .iter()
        .map(|m| format!("{}  ->  {}/{}", m.source, m.dest_folder, m.final_name))
        .collect();
    section(&mut lines, "Files moved", &move_lines);

    if plan.keep_structure {
        let removed: &[String] = match result {
            None => &plan.removable_source_dirs,
            Some(r) => &r.removed_source_dirs,
        };
        section(&mut lines, "Source folders removed", removed);
    }

    let skip_lines: Vec<String> = plan
        .skipped
        .iter()
        .map(|s| format!("{}  ({})", s.name, s.reason))
        .collect();
    section(&mut lines, "Skipped", &skip_lines);

    let conflicts: Vec<&PlannedMove> = moves.iter().filter(|m| m.renamed).collect();
    let mut issues: Vec<String> = conflicts
        .iter()
        .map(|m| {
            format!(
                "conflict: \"{}\" already existed in {}; moved as \"{}\"",
                m.source, m.dest_folder, m.final_name
            )
        })
        .collect();
    issues.extend(
        errors
            .iter()
            .map(|e| format!("error: could not move \"{}\": {}", e.source, e.message)),
    );
    section(&mut lines, "Issues", &issues);

    lines.push(format!(
        "Totals: {} moved, {} created, {}, {}",
        count(moves.len(), "file"),
        count(plan.new_folders.len(), "folder"),
        count(conflicts.len(), "conflict"),
        count(errors.len(), "error"),
    ));
    lines.join("\n")
}

/// Render the undo report. `result` is `None` for dry runs.
pub fn format_undo_report(
    folder: &str,
    plan: &UndoPlan,
    result: Option<&UndoResult>,
    dry_run: bool,
) -> String {
    let (restores, removed, errors): (&[PlannedRestore], Vec<String>, Vec<MoveError>) = match result {
        None => (
            &plan.restores,
            plan.removable_folders.clone(),
            plan.missing
                .iter()
                .map(|m| MoveError {
                    source: format!("{}/{}", m.dest_folder, m.final_name),
                    message: "file not found".to_string(),
                })
                .collect(),
        ),
        Some(r) => (&r.restored, r.removed_folders.clone(), r.errors.clone()),
    };

    let mut lines: Vec<String> = Vec::new();
    if dry_run {
        lines.push("DRY RUN - no changes made".to_string());
    }
    lines.push(format!("Undoing last run in: {folder}"));
    lines.push(String::new());

    section(&mut lines, "Folders removed", &removed);
    let restore_lines: Vec<String> = restores
        .iter()
        .map(|r| format!("{}/{}  ->  {}", r.dest_folder, r.final_name, r.restore_name))
        .collect();
    section(&mut lines, "Files restored", &restore_lines);

    let conflicts: Vec<&PlannedRestore> = restores.iter().filter(|r| r.renamed).collect();
    let mut issues: Vec<String> = conflicts
        .iter()
        .map(|r| {
            format!(
                "conflict: \"{}\" already existed; restored as \"{}\"",
                r.source, r.restore_name
            )
        })
        .collect();
    issues.extend(
        errors
            .iter()
            .map(|e| format!("error: could not restore \"{}\": {}", e.source, e.message)),
    );
    section(&mut lines, "Issues", &issues);

    lines.push(format!(
        "Totals: {} restored, {} removed, {}, {}",
        count(restores.len(), "file"),
        count(removed.len(), "folder"),
        count(conflicts.len(), "conflict"),
        count(errors.len(), "error"),
    ));
    lines.join("\n")
}
