//! Shared BDD support: the typed World and the full step registry, driving the
//! real `file-organizer` binary as a subprocess. Used by both feature sets.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use gherkin_cargo_test::StepRegistry;

const BIN: &str = env!("CARGO_BIN_EXE_file-organizer");
static SEQ: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, PartialEq, Eq)]
enum SnapVal {
    File(Vec<u8>),
    Dir,
}

pub struct World {
    root: PathBuf,
    workspace: PathBuf,
    target: PathBuf,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    snapshot: Option<BTreeMap<String, SnapVal>>,
}

impl Default for World {
    fn default() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fo-bdd-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        World {
            root,
            workspace: workspace.clone(),
            target: workspace,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            snapshot: None,
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl World {
    fn write(&self, rel: &str, content: &str) {
        let path = self.workspace.join(rel);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn run(&mut self, extra: &[&str]) {
        let out = Command::new(BIN)
            .arg(&self.target)
            .args(extra)
            .output()
            .expect("spawn file-organizer");
        self.exit_code = out.status.code();
        self.stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        self.stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    }

    fn take_snapshot(&mut self) {
        let mut map = BTreeMap::new();
        snapshot_into(&self.workspace, &self.workspace, &mut map);
        self.snapshot = Some(map);
    }
}

fn snapshot_into(root: &std::path::Path, dir: &std::path::Path, out: &mut BTreeMap<String, SnapVal>) {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            out.insert(rel, SnapVal::Dir);
            snapshot_into(root, &path, out);
        } else {
            let bytes = std::fs::read(&path).unwrap_or_default();
            out.insert(rel, SnapVal::File(bytes));
        }
    }
}

fn current_snapshot(w: &World) -> BTreeMap<String, SnapVal> {
    let mut map = BTreeMap::new();
    snapshot_into(&w.workspace, &w.workspace, &mut map);
    map
}

/// Registers every organizer step. A superset shared by both feature sets;
/// unused patterns for a given set are harmless (only *unbound* steps fail).
pub fn organizer_steps(reg: &mut StepRegistry<World>) {
    // --- Given: workspace setup ---------------------------------------------
    reg.define(r"^the target path does not exist$", |ctx, _, _| {
        ctx.world.target = ctx.world.root.join("does_not_exist");
    });
    reg.define(r"^the target path is a file$", |ctx, _, _| {
        let target = ctx.world.root.join("target.txt");
        std::fs::write(&target, "not a folder").expect("write target file");
        ctx.world.target = target;
    });
    reg.define(r#"^the workspace contains a file named "([^"]+)"$"#, |ctx, args, _| {
        let content = format!("content-of-{}", args[0]);
        ctx.world.write(&args[0], &content);
    });
    reg.define(
        r#"^the workspace contains a file named "([^"]+)" with content "([^"]*)"$"#,
        |ctx, args, _| ctx.world.write(&args[0], &args[1]),
    );
    reg.define(
        r#"^the workspace contains a subfolder named "([^"]+)" containing a file named "([^"]+)"$"#,
        |ctx, args, _| {
            let rel = format!("{}/{}", args[0], args[1]);
            let content = format!("content-of-{}", args[1]);
            ctx.world.write(&rel, &content);
        },
    );
    reg.define(
        r#"^the workspace contains a subfolder named "([^"]+)" containing a file named "([^"]+)" with content "([^"]*)"$"#,
        |ctx, args, _| {
            let rel = format!("{}/{}", args[0], args[1]);
            ctx.world.write(&rel, &args[2]);
        },
    );
    reg.define(r#"^the workspace contains a nested file named "([^"]+)"$"#, |ctx, args, _| {
        let content = format!("content-of-{}", args[0]);
        ctx.world.write(&args[0], &content);
    });
    reg.define(
        r#"^the workspace contains a nested file named "([^"]+)" with content "([^"]*)"$"#,
        |ctx, args, _| ctx.world.write(&args[0], &args[1]),
    );
    reg.define(r#"^the workspace contains an empty subfolder named "([^"]+)"$"#, |ctx, args, _| {
        std::fs::create_dir_all(ctx.world.workspace.join(&args[0])).expect("mkdir");
    });
    reg.define(
        r#"^the workspace contains a symlink named "([^"]+)" pointing to "([^"]+)"$"#,
        |ctx, args, _| {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&args[1], ctx.world.workspace.join(&args[0]))
                .expect("create symlink");
        },
    );
    reg.define(r#"^the workspace contains a fifo named "([^"]+)"$"#, |ctx, args, _| {
        let path = ctx.world.workspace.join(&args[0]);
        let status = Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("run mkfifo");
        assert!(status.success(), "mkfifo failed");
    });
    reg.define(r"^the workspace contains a corrupt manifest$", |ctx, _, _| {
        ctx.world
            .write(".file_organizer_manifest.json", "NOT JSON {");
    });

    // --- When: invocations --------------------------------------------------
    reg.define(r"^I run the organizer on the target$", |ctx, _, _| ctx.world.run(&[]));
    reg.define(r"^I run the organizer on the target again$", |ctx, _, _| ctx.world.run(&[]));
    reg.define(r"^I run the organizer on the target with --dry-run$", |ctx, _, _| {
        ctx.world.take_snapshot();
        ctx.world.run(&["--dry-run"]);
    });
    reg.define(r"^I run the organizer on the target with --recursive$", |ctx, _, _| {
        ctx.world.run(&["--recursive"]);
    });
    reg.define(
        r"^I run the organizer on the target with --recursive and --keep-structure$",
        |ctx, _, _| ctx.world.run(&["--recursive", "--keep-structure"]),
    );
    reg.define(r"^I run the organizer on the target with --keep-structure only$", |ctx, _, _| {
        ctx.world.run(&["--keep-structure"]);
    });
    reg.define(
        r"^I run the organizer on the target with --recursive, --keep-structure and --dry-run$",
        |ctx, _, _| {
            ctx.world.take_snapshot();
            ctx.world.run(&["--recursive", "--keep-structure", "--dry-run"]);
        },
    );
    reg.define(
        r"^I run the organizer on the target with --recursive and --dry-run$",
        |ctx, _, _| {
            ctx.world.take_snapshot();
            ctx.world.run(&["--recursive", "--dry-run"]);
        },
    );
    reg.define(r"^I run the organizer on the target with --undo$", |ctx, _, _| {
        ctx.world.run(&["--undo"]);
    });
    reg.define(r"^I run the organizer on the target with --undo and --dry-run$", |ctx, _, _| {
        ctx.world.run(&["--undo", "--dry-run"]);
    });
    reg.define(r"^I run the organizer on the target with --version$", |ctx, _, _| {
        ctx.world.run(&["--version"]);
    });
    reg.define(r#"^the folder "([^"]+)" is deleted from the workspace$"#, |ctx, args, _| {
        std::fs::remove_dir(ctx.world.workspace.join(&args[0])).expect("rmdir");
    });
    reg.define(
        r#"^the workspace gains a file named "([^"]+)" with content "([^"]*)"$"#,
        |ctx, args, _| ctx.world.write(&args[0], &args[1]),
    );
    reg.define(r#"^the file "([^"]+)" is deleted from the workspace$"#, |ctx, args, _| {
        std::fs::remove_file(ctx.world.workspace.join(&args[0])).expect("unlink");
    });

    // --- Then: assertions ---------------------------------------------------
    reg.define(r"^the exit code is (\d+)$", |ctx, args, _| {
        let want: i32 = args[0].parse().expect("exit code");
        assert_eq!(
            ctx.world.exit_code,
            Some(want),
            "expected exit {want}, got {:?}\nstdout:\n{}\nstderr:\n{}",
            ctx.world.exit_code,
            ctx.world.stdout,
            ctx.world.stderr
        );
    });
    reg.define(r#"^stderr contains "([^"]+)"$"#, |ctx, args, _| {
        assert!(
            ctx.world.stderr.contains(&args[0]),
            "missing {:?} in stderr:\n{}",
            args[0],
            ctx.world.stderr
        );
    });
    reg.define(r#"^the report contains "([^"]+)"$"#, |ctx, args, _| {
        assert!(
            ctx.world.stdout.contains(&args[0]),
            "missing {:?} in report:\n{}",
            args[0],
            ctx.world.stdout
        );
    });
    reg.define(r#"^the report does not contain "([^"]+)"$"#, |ctx, args, _| {
        assert!(
            !ctx.world.stdout.contains(&args[0]),
            "unexpected {:?} in report:\n{}",
            args[0],
            ctx.world.stdout
        );
    });
    reg.define(r#"^the workspace contains a folder named "([^"]+)"$"#, |ctx, args, _| {
        assert!(
            ctx.world.workspace.join(&args[0]).is_dir(),
            "folder {:?} missing",
            args[0]
        );
    });
    reg.define(r#"^the workspace does not contain a folder named "([^"]+)"$"#, |ctx, args, _| {
        assert!(
            !ctx.world.workspace.join(&args[0]).exists(),
            "folder {:?} should not exist",
            args[0]
        );
    });
    reg.define(r#"^the file "([^"]+)" exists in the workspace$"#, |ctx, args, _| {
        assert!(
            ctx.world.workspace.join(&args[0]).is_file(),
            "file {:?} missing",
            args[0]
        );
    });
    reg.define(r#"^the file "([^"]+)" does not exist in the workspace$"#, |ctx, args, _| {
        assert!(
            !ctx.world.workspace.join(&args[0]).exists(),
            "file {:?} should not exist",
            args[0]
        );
    });
    reg.define(
        r#"^the file "([^"]+)" in the workspace has content "([^"]*)"$"#,
        |ctx, args, _| {
            let path = ctx.world.workspace.join(&args[0]);
            assert!(path.is_file(), "file {:?} missing", args[0]);
            let got = std::fs::read_to_string(&path).expect("read file");
            assert_eq!(got, args[1], "content mismatch for {:?}", args[0]);
        },
    );
    reg.define(r"^the workspace is unchanged$", |ctx, _, _| {
        let before = ctx.world.snapshot.as_ref().expect("snapshot was taken");
        let now = current_snapshot(&ctx.world);
        assert!(&now == before, "workspace changed during a dry run");
    });
    reg.define(r"^the report sections appear in order$", |ctx, _, _| {
        let order = [
            "Folders created:",
            "Files moved:",
            "Skipped:",
            "Issues:",
            "Totals:",
        ];
        let mut positions = Vec::new();
        for section in order {
            let pos = ctx
                .world
                .stdout
                .find(section)
                .unwrap_or_else(|| panic!("missing section {section:?} in report:\n{}", ctx.world.stdout));
            positions.push(pos);
        }
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted, "sections out of order:\n{}", ctx.world.stdout);
    });
    reg.define(r"^the Issues section shows none$", |ctx, _, _| {
        assert!(
            ctx.world.stdout.contains("Issues:\n  none"),
            "Issues section not empty:\n{}",
            ctx.world.stdout
        );
    });
}
