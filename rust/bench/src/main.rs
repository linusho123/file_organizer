//! stdin -> plan report -> stdout. Runs natively and under wasmtime
//! (wasm32-wasip1). The wasm2c endpoints use the library's C ABI instead.

use std::io::{Read, Write};

use file_organizer_bench::run_bench;

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).expect("read stdin");
    let out = run_bench(&input);
    let mut stdout = std::io::stdout();
    stdout.write_all(out.as_bytes()).expect("write stdout");
    stdout.write_all(b"\n").expect("write newline");
}
