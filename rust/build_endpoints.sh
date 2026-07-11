#!/usr/bin/env bash
# Reproducibly build all four compiler endpoints of the file-organizer core.
#
#   1. rust-native        cargo (x86_64-unknown-linux-gnu)
#   2. wasmtime           cargo --target wasm32-wasip1, run via wasmtime
#   3. wasm2c-clang       wasm32-unknown-unknown cdylib -> wasm2c -> clang (ELF)
#   4. wasm2c-cosmocc     ...same C sources -> cosmocc (fat APE / universal)
#
# Prereqs (all local on this machine):
#   cargo + rustup targets wasm32-wasip1, wasm32-unknown-unknown
#   wasm2c/wat2wasm + wasm-rt runtime : ~/gitrepos/wabt
#   cosmocc toolchain                 : rust/bench_build/cosmocc-dist (from cosmo.zip)
#   wasmtime                          : ~/.wasmtime/bin
set -euo pipefail
cd "$(dirname "$0")"
export CARGO_TARGET_DIR="$PWD/target"

WABT="$HOME/gitrepos/wabt/wasm2c"
WASM2C="$HOME/gitrepos/wabt/build/wasm2c"
COSMOCC="$PWD/bench_build/cosmocc-dist/bin/cosmocc"
ASSIM="$PWD/bench_build/cosmocc-dist/bin/assimilate"
mkdir -p bench_build/wasm2c

echo "==> 1/4 rust-native (release)"
cargo build --release -p file-organizer-cli
cargo build --release -p file-organizer-bench --bin bench

echo "==> 2/4 wasmtime (wasm32-wasip1)"
cargo build --release -p file-organizer-cli   --bin file-organizer --target wasm32-wasip1
cargo build --release -p file-organizer-bench --bin bench          --target wasm32-wasip1

echo "==> 3/4 + 4/4 wasm2c path: cdylib -> wasm2c -> C"
cargo build --release -p file-organizer-bench --lib --target wasm32-unknown-unknown
"$WASM2C" target/wasm32-unknown-unknown/release/file_organizer_bench.wasm \
  -o bench_build/wasm2c/bench.c

echo "    clang -> native ELF"
clang -O3 -I"$WABT" -Ibench_build/wasm2c \
  bench_c/main.c bench_build/wasm2c/bench.c \
  "$WABT/wasm-rt-impl.c" "$WABT/wasm-rt-mem-impl.c" -lm \
  -o bench_build/bench_wasm2c_clang

echo "    cosmocc -> fat APE (x86_64 + aarch64)"
"$COSMOCC" -O3 -I"$WABT" -Ibench_build/wasm2c \
  bench_c/main.c bench_build/wasm2c/bench.c \
  "$WABT/wasm-rt-impl.c" "$WABT/wasm-rt-mem-impl.c" \
  -o bench_build/bench_wasm2c_cosmocc

echo "    assimilate a copy -> native x86_64 ELF (isolates codegen from APE loader)"
cp -f bench_build/bench_wasm2c_cosmocc bench_build/bench_wasm2c_cosmocc_native
"$ASSIM" -f bench_build/bench_wasm2c_cosmocc_native

echo "==> done. artifacts in bench_build/:"
ls -la bench_build/bench_wasm2c_* target/release/bench 2>/dev/null | awk '{print "   ", $5, $NF}'
