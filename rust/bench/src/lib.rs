//! Pure-core planning workload, endpoint-independent.
//!
//! `run_bench` takes a serialized directory listing and returns the plan
//! report as a string — no filesystem, no clock, no randomness. It is the
//! single code path benchmarked across all four compiler endpoints:
//!   * native (x86_64)               — via the `bench` binary
//!   * wasm32-wasip1 under wasmtime  — via the `bench` binary
//!   * wasm32-unknown-unknown        — via wasm2c + a C harness (clang, cosmocc)
//!
//! Input format (stdin / input buffer):
//!   line 1:  "<recursive> <keep_structure>"   e.g. "1 0"
//!   line n:  "<K>\t<relative/posix/path>"      K in {F, D, L, O}
//!
//! Output: the organize report for that listing (deterministic — usable as an
//! N-version checksum across endpoints).

use file_organizer_core::{build_plan, format_report, Entry, Kind};

pub fn run_bench(input: &[u8]) -> String {
    let text = String::from_utf8_lossy(input);
    let mut lines = text.lines();

    let header = lines.next().unwrap_or("0 0");
    let mut hp = header.split_whitespace();
    let recursive = hp.next().map_or(false, |t| t == "1");
    let keep_structure = hp.next().map_or(false, |t| t == "1");

    let mut entries: Vec<Entry> = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (k, path) = match line.split_once('\t') {
            Some(pair) => pair,
            None => continue,
        };
        let kind = match k {
            "F" => Kind::File,
            "D" => Kind::Dir,
            "L" => Kind::Symlink,
            _ => Kind::Other,
        };
        entries.push(Entry::new(path, kind));
    }

    let plan = build_plan(&entries, recursive, keep_structure);
    format_report("/bench", &plan, None, false)
}

// --- C ABI for the wasm2c endpoints -----------------------------------------
// wasm32-unknown-unknown has no stdin; the C harness owns all I/O and drives
// these exports across the module's linear memory (the classic wasm2c pattern).

/// Allocate `len` bytes in wasm linear memory; returns the offset. The harness
/// fills it with the input listing before calling [`bench_run`].
#[no_mangle]
pub extern "C" fn bench_alloc(len: usize) -> *mut u8 {
    let mut v = vec![0u8; len];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Run the workload over `in_len` bytes at `in_ptr`; writes the output length
/// through `out_len` and returns the offset of the output bytes.
///
/// # Safety
/// `in_ptr`/`in_len` must describe a valid buffer previously produced by
/// [`bench_alloc`]; `out_len` must be a valid writable pointer.
#[no_mangle]
pub unsafe extern "C" fn bench_run(in_ptr: *const u8, in_len: usize, out_len: *mut usize) -> *mut u8 {
    let input = std::slice::from_raw_parts(in_ptr, in_len);
    let out = run_bench(input).into_bytes();
    *out_len = out.len();
    let mut boxed = out.into_boxed_slice();
    let p = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    p
}
