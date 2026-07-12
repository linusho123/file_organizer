#!/usr/bin/env bash
# Build the universal file-organizer APE.
#
#   reactor (Rust, pure) --cargo--> wasm32-unknown-unknown  (NO WASI)
#                        --wasm2c--> C
#   driver.c (real I/O) + reactor.c + vendored wasm-rt  --cosmocc--> file-organizer.com
#
# The reactor holds all logic; driver.c only snapshots the dir and replays ops.
# Tool locations are overridable via env so CI can point at its own downloads.
set -euo pipefail
cd "$(dirname "$0")/.."            # -> rust/
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"

WASM2C="${WASM2C:-$HOME/gitrepos/wabt/build/wasm2c}"
COSMOCC="${COSMOCC:-$PWD/bench_build/cosmocc-dist/bin/cosmocc}"
OUT="${OUT:-$PWD/bench_build}"
# The wasm-rt runtime MUST match the wasm2c that generates the C. The vendored
# copy matches the local WABT; CI overrides WASM_RT_DIR to its own build.
RT="${WASM_RT_DIR:-$PWD/ape/vendor/wasm-rt}"
mkdir -p "$OUT/reactor_c"

echo "== 1/3 reactor -> wasm32-unknown-unknown =="
cargo build --release -p file-organizer-reactor --lib --target wasm32-unknown-unknown

echo "== 2/3 wasm2c =="
"$WASM2C" target/wasm32-unknown-unknown/release/file_organizer_reactor.wasm \
    -n reactor -o "$OUT/reactor_c/reactor.c"

echo "== 3/3 cosmocc -> file-organizer.com (fat APE; MMAP=0 for honest portability) =="
"$COSMOCC" -O3 -DWASM_RT_USE_MMAP=0 -DWASM_RT_MODULE_PREFIX= \
    -I"$RT" -I"$OUT/reactor_c" \
    ape/driver.c "$OUT/reactor_c/reactor.c" \
    "$RT/wasm-rt-impl.c" "$RT/wasm-rt-mem-impl.c" \
    -o "$OUT/file-organizer.com"

echo "-> $OUT/file-organizer.com ($(stat -c%s "$OUT/file-organizer.com") bytes)"
