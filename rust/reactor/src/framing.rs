//! Byte framing shared with the C driver. Keyword header lines + length-
//! prefixed blobs; counted lists for args/entries/ops. Paths and args are
//! assumed newline-free; blobs (manifest, stdout, stderr) are byte-exact.
//!
//! INPUT                              OUTPUT
//!   ARGS <n>\n                         EXIT <int>\n
//!   <arg>\n            x n             STDOUT <len>\n<bytes>
//!   EXISTS <0|1>\n                     STDERR <len>\n<bytes>
//!   ISDIR <0|1>\n                      OPS <n>\n
//!   NOW <str>\n                          MKDIR\t<path>\n
//!   FOLDER <str>\n                       MOVE\t<from>\t<to>\n
//!   ENTRIES <n>\n                        RMDIR\t<path>\n
//!   <K>\t<rel>\n       x n               DELETE\t<path>\n
//!   MANIFEST <len|-1>\n                  WRITE\t<path>\t<len>\n<bytes>
//!   <manifest bytes>

use file_organizer_core::{Entry, Kind};

use crate::{run_typed, Input, Op, Output};

struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Cur<'a> {
    fn line(&mut self) -> &'a [u8] {
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i] != b'\n' {
            self.i += 1;
        }
        let line = &self.b[start..self.i];
        if self.i < self.b.len() {
            self.i += 1; // consume '\n'
        }
        line
    }
    fn take(&mut self, n: usize) -> &'a [u8] {
        let end = (self.i + n).min(self.b.len());
        let s = &self.b[self.i..end];
        self.i = end;
        s
    }
}

fn after_space(line: &[u8]) -> &[u8] {
    match line.iter().position(|&c| c == b' ') {
        Some(p) => &line[p + 1..],
        None => &[],
    }
}
fn int_val(line: &[u8]) -> i64 {
    std::str::from_utf8(after_space(line))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}
fn str_val(line: &[u8]) -> String {
    String::from_utf8_lossy(after_space(line)).into_owned()
}

pub fn parse_input(bytes: &[u8]) -> Input {
    let mut c = Cur { b: bytes, i: 0 };
    let argc = int_val(c.line()).max(0) as usize;
    let mut args = Vec::with_capacity(argc);
    for _ in 0..argc {
        args.push(String::from_utf8_lossy(c.line()).into_owned());
    }
    let exists = int_val(c.line()) != 0;
    let is_dir = int_val(c.line()) != 0;
    let now = str_val(c.line());
    let folder_display = str_val(c.line());
    let en = int_val(c.line()).max(0) as usize;
    let mut entries = Vec::with_capacity(en);
    for _ in 0..en {
        let line = c.line();
        if let Some(p) = line.iter().position(|&ch| ch == b'\t') {
            let kind = match line[0] {
                b'F' => Kind::File,
                b'D' => Kind::Dir,
                b'L' => Kind::Symlink,
                _ => Kind::Other,
            };
            let rel = String::from_utf8_lossy(&line[p + 1..]).into_owned();
            entries.push(Entry::new(rel, kind));
        }
    }
    let mlen = int_val(c.line());
    let manifest = if mlen >= 0 {
        Some(c.take(mlen as usize).to_vec())
    } else {
        None
    };
    Input { args, exists, is_dir, now, folder_display, entries, manifest }
}

pub fn serialize_output(o: &Output) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(format!("EXIT {}\n", o.exit).as_bytes());
    b.extend_from_slice(format!("STDOUT {}\n", o.stdout.len()).as_bytes());
    b.extend_from_slice(o.stdout.as_bytes());
    b.extend_from_slice(format!("STDERR {}\n", o.stderr.len()).as_bytes());
    b.extend_from_slice(o.stderr.as_bytes());
    b.extend_from_slice(format!("OPS {}\n", o.ops.len()).as_bytes());
    for op in &o.ops {
        match op {
            Op::Mkdir(p) => b.extend_from_slice(format!("MKDIR\t{p}\n").as_bytes()),
            Op::Move { from, to } => {
                b.extend_from_slice(format!("MOVE\t{from}\t{to}\n").as_bytes())
            }
            Op::Rmdir(p) => b.extend_from_slice(format!("RMDIR\t{p}\n").as_bytes()),
            Op::DeleteFile(p) => b.extend_from_slice(format!("DELETE\t{p}\n").as_bytes()),
            Op::WriteFile { path, content } => {
                b.extend_from_slice(format!("WRITE\t{path}\t{}\n", content.len()).as_bytes());
                b.extend_from_slice(content);
            }
        }
    }
    b
}

/// Byte-in, byte-out entry point used by the C driver (via `reactor_run`).
pub fn run(input: &[u8]) -> Vec<u8> {
    serialize_output(&run_typed(parse_input(input)))
}
