//! Native driver: snapshot the target folder, hand it to the reactor, and
//! replay the ops the reactor returns. All organizing logic lives in the
//! reactor — this file only does real filesystem I/O. The same reactor drives
//! the universal (cosmocc/APE) build through a C harness.

use std::path::Path;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

use file_organizer_core::{Entry, Kind, MANIFEST_NAME};
use file_organizer_reactor::{run_typed, Input, Op};

fn main() {
    exit(run(std::env::args().skip(1).collect()));
}

fn run(args: Vec<String>) -> i32 {
    // The reactor parses args authoritatively; we only need the folder up front
    // to snapshot it (first non-flag argument).
    let folder_arg = args.iter().find(|a| !a.starts_with('-')).cloned();

    let mut exists = false;
    let mut is_dir = false;
    let mut folder_display = folder_arg.clone().unwrap_or_default();
    let mut entries: Vec<Entry> = Vec::new();
    let mut manifest: Option<Vec<u8>> = None;
    let mut base = None;

    if let Some(f) = &folder_arg {
        let p = Path::new(f);
        exists = p.exists();
        is_dir = p.is_dir();
        if exists && is_dir {
            let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            entries = snapshot(&abs);
            let mpath = abs.join(MANIFEST_NAME);
            if mpath.is_file() {
                manifest = std::fs::read(&mpath).ok();
            }
            folder_display = abs.display().to_string();
            base = Some(abs);
        }
    }

    let out = run_typed(Input {
        args,
        exists,
        is_dir,
        now: iso8601_now(),
        folder_display,
        entries,
        manifest,
    });

    if !out.stdout.is_empty() {
        print!("{}", out.stdout);
    }
    if !out.stderr.is_empty() {
        eprint!("{}", out.stderr);
    }
    if let Some(base) = base {
        execute(&base, &out.ops);
    }
    out.exit
}

// --- filesystem: snapshot + op replay ---------------------------------------

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

fn move_file(src: &Path, dst: &Path) -> Result<(), ()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => match std::fs::copy(src, dst) {
            Ok(_) => std::fs::remove_file(src).map_err(|_| ()),
            Err(_) => Err(()),
        },
    }
}

fn execute(base: &Path, ops: &[Op]) {
    for op in ops {
        match op {
            Op::Mkdir(p) => {
                let _ = std::fs::create_dir_all(base.join(p));
            }
            Op::Move { from, to } => {
                let dst = base.join(to);
                if let Some(parent) = dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = move_file(&base.join(from), &dst);
            }
            Op::Rmdir(p) => {
                let _ = std::fs::remove_dir(base.join(p));
            }
            Op::WriteFile { path, content } => {
                let _ = std::fs::write(base.join(path), content);
            }
            Op::DeleteFile(p) => {
                let _ = std::fs::remove_file(base.join(p));
            }
        }
    }
}

// --- timestamp for the manifest (UTC ISO-8601; not asserted by any test) ----

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
