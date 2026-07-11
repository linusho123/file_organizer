# File Organizer — BDD-driven Rust port & compiler-endpoint benchmark

*A data point for the `gherkin-cargo-test` BDD workflow: take a working Python
CLI, extract its feature files as the language-neutral contract, port the tool
to Rust driven by those features, compile the Rust core to four different
compiler endpoints, and measure how they compare to the Python original.*

Branch: `rust-port-bdd-experiment` · worktree: `rust-port-features-original`

---

## 1. What was done

| Step | Result |
|---|---|
| **1. Branch** | `rust-port-bdd-experiment` off `master` (v0.5.0). |
| **2. Review feature files vs code** | 8/9 features parse under the strict micro-grammar; **`collisions.feature` was rejected**, and **5 real behaviors were undocumented**. Produced two feature sets (below). |
| **3. Rust port via gherkin-cargo-test** | Pure `core` crate + native `cli`; the same `.feature` files drive it. |
| **4. Both feature sets green** | Set A (original) **68/68**, Set B (augmented) **75/75**; a Python↔Rust differential is **byte-identical**. Worktree builds the port off the original set independently. |
| **5. Four compiler endpoints** | rust-native, wasmtime (wasm32-wasip1), wasm2c→clang, wasm2c→cosmocc (universal APE). All **byte-identical output**. |
| **6. Benchmark + report** | Two tracks (real-FS end-to-end; pure-core planning), cold-start / memory / speed, medians with 95% bootstrap CIs. This document. |

---

## 2. Phase 2 — Feature-file review

The Python package is healthy: **60 pytest-bdd scenarios + unit tests pass, 98.7 %
coverage**. The feature files faithfully track the code *where they exist*. Two
findings mattered for the port:

### 2a. One feature file could not drive `gherkin-cargo-test` as-is

`gherkin-cargo-test` uses a deliberately strict "micro-grammar" that turns any
unrecognised construct into a hard `file:line` error (its anti-"false-green"
doctrine). Running its own `parse` example over the corpus:

```
REJECT  collisions.feature:2: step before any Scenario or Background
```

The `Feature:` narrative began *"When a file being moved…"* — and `When ` is a
step keyword. The parser correctly refused to silently treat a step as prose.
**Fix:** reword the narrative (no semantic change). This is exactly the kind of
latent ambiguity a strict runner is designed to surface.

### 2b. Five behaviors the implementation has but no scenario documented

Each confirmed by probing the real Python CLI:

| Behavior | Actual result | Spec ref |
|---|---|---|
| `weird.` / `a.b.` (trailing dot) | → `NO_EXTENSION_Files` | FR-6 |
| top-level symlink | skipped `(symlink)`, never moved | FR-5 |
| corrupt manifest + `--undo` | exit **2**, `could not read manifest:` | FR-27 |
| `--version` | prints program + version, exit 0 | FR-17 |
| fifo / non-regular file | skipped `(not a regular file)` | organizer.py:144 (uncovered) |

### The two feature sets

* **Set A — `features_original/`** (8 files, 59 scenarios): the original corpus,
  with only the `collisions.feature` narrative reworded so it parses. Packaging
  (`packaging.feature`, PyPI/CI-specific) is parked — it is not part of the
  behavioral contract.
* **Set B — `features_augmented/`** (9 files, 65 scenarios): Set A **+**
  `edge_behaviors.feature` pinning down the five behaviors above.

The port is built and verified against **both**.

---

## 3. Phases 3–4 — The Rust port and its equivalence to Python

**Architecture (endpoint independence by construction):**

* **`core`** — pure planning/undo/report logic. *No `std::fs`, no clock, no
  randomness.* Input is a snapshot of directory entries; output is a plan or a
  report string. Report strings are **byte-for-byte identical** to Python's
  (same ASCII `->`, same `Totals:` wording, same conflict phrasing). This is the
  portable algorithm that every endpoint compiles.
* **`cli`** — native front-end: argument parsing, exit codes, the real
  filesystem walk/moves, and manifest JSON. Thin; delegates all logic to `core`.
* **`bench`** — the same `core` behind a stdin→stdout workload, for the endpoint
  benchmark. One code path, four compilers.

`gherkin-cargo-test` runs as the host harness (one typed `World`, step defs that
spawn the real binary). Results:

```
Set A (features_original) :  68 passed; 0 failed      (59 scenarios + 9 guards)
Set B (features_augmented):  75 passed; 0 failed      (65 scenarios + 10 guards)
```

**N-version differential** (the framework's core idea): run Python and Rust on
byte-identical workspaces and diff. Across organize / dry-run / undo / recursive
/ keep-structure / collisions, **reports and resulting file trees are identical**:

```
ALL DIFFERENTIAL CHECKS IDENTICAL
```

**Worktree dual-build (step 4):** the port satisfies both contracts. Building in
an isolated `git worktree` with the *original* set as the acceptance layer:
`68/68` green — the modifications did not "drive" the pass; the un-augmented
contract holds on its own.

---

## 4. Phase 5 — Four compiler endpoints, one core

```
             cargo (native)  ──────────────────────────────►  rust-native  (ELF)
file-organizer-core ─► bench ─► cargo --target wasm32-wasip1 ─► wasmtime     (JIT .wasm)
                                └► cargo --target wasm32-unknown-unknown (cdylib)
                                        └► wasm2c ─► C ─┬─ clang   ──────────►  wasm2c-clang  (ELF)
                                                        └─ cosmocc ──────────►  wasm2c-cosmocc (fat APE)
```

The wasm2c endpoints use a ~60-line C harness (`bench_c/main.c`) that owns all
I/O and drives the module across its linear memory — so the wasm needs **no WASI
syscalls at all** (only stdin/stdout, handled in C). The cosmocc output is an
Actually-Portable Executable carrying both x86-64 and aarch64 slices.

**Equivalence — identical output at every size (md5):**

| workload | native | wasmtime | wasm2c-clang | wasm2c-cosmocc | agree |
|---|---|---|---|---|---|
| 1 000 files | `68d8182b…` | `68d8182b…` | `68d8182b…` | `68d8182b…` | ✅ |
| 10 000 files | `337d92ba…` | `337d92ba…` | `337d92ba…` | `337d92ba…` | ✅ |
| 100 000 files | `278ab7c8…` | `278ab7c8…` | `278ab7c8…` | `278ab7c8…` | ✅ |

## 5. Benchmark results

```
AMD Ryzen 5 PRO 8500GE  ×12 threads · 14.2 GiB · Linux 7.1.3 · work trees on /dev/shm
rustc 1.97 · clang 21 · cosmocc(GCC) 14.1 · wasmtime 46 · wasm2c 1.0.41 (WABT) · CPython 3.13.13
25 repetitions (100k workloads use ≥8); medians reported; ratios with 95% bootstrap CIs.
```

### Artifact sizes

| Artifact | Size (KiB) |
| --- | ---: |
| rust-native cli | 538.1 |
| rust-native bench | 405.3 |
| wasip1 cli `.wasm` | 334.5 |
| wasip1 bench `.wasm` | 180.9 |
| wasm2c-clang (ELF) | 233.8 |
| wasm2c-cosmocc (fat APE) | 896.2 |

### Track B — pure planning core (stdin→stdout, no filesystem)

The *same* Rust core compiled four ways; byte-identical output at every size. `-ape` = the universal binary run via the APE loader; `-elf` = its assimilated x86-64 slice (isolates codegen from loader overhead).

**Wall clock — median ms**

| workload | native | wasmtime | wasm2c-clang | cosmocc-ape | cosmocc-elf |
| --- | ---: | ---: | ---: | ---: | ---: |
| cold (5 files) | 0.36 | 4.25 | 0.35 | 0.21 | 0.19 |
| 10 000 | 15.5 | 22.4 | 16.1 | 17.8 | 17.4 |
| 100 000 | 178 | 220 | 176 | 190 | 187 |

**Peak memory — MiB (max-RSS)**

| workload | native | wasmtime | wasm2c-clang | cosmocc-ape | cosmocc-elf |
| --- | ---: | ---: | ---: | ---: | ---: |
| cold | 2.4 | 18.6 | 2.3 | 0.4 | 0.4 |
| 10 000 | 8.2 | 23.5 | 6.7 | 5.1 | 5.1 |
| 100 000 | 60.9 | 66.8 | 49.6 | 47.9 | 47.8 |

**Slowdown vs native at 100 k** (median ratio, 95% bootstrap CI)

| endpoint | ×slower | CI95 |
| --- | ---: | --- |
| native | 1.00 | [0.98, 1.02] |
| wasm2c-clang | **0.99** | [0.97, 1.03] |
| wasm2c-cosmocc-elf | 1.05 | [1.03, 1.08] |
| wasm2c-cosmocc-ape | 1.06 | [1.04, 1.11] |
| wasmtime | 1.24 | [1.21, 1.30] |

### Track A — real end-to-end CLI (organizes a real tree, `--recursive`)

Python vs the Rust port (native, and wasm32-wasip1 under wasmtime), all doing genuine filesystem moves. `N` = files organized.

**Wall clock — median ms**

| N (files) | python | rust-native | wasmtime-wasip1 |
| --- | ---: | ---: | ---: |
| 1 (cold start) | 26.8 | **0.54** | 4.91 |
| 2 000 | 77.3 | **13.8** | 32.0 |
| 20 000 | 510 | **121** | 522 |

**Peak memory — MiB (max-RSS)**

| N (files) | python | rust-native | wasmtime-wasip1 |
| --- | ---: | ---: | ---: |
| 1 | 25.0 | 2.5 | 20.2 |
| 2 000 | 26.8 | 6.9 | 22.9 |
| 20 000 | 41.9 | 46.2 | 54.0 |

**Speedup vs Python** (median ratio, 95% bootstrap CI)

| N | rust-native | wasmtime-wasip1 |
| --- | ---: | ---: |
| 1 | **49.8×** [44.2, 52.2] | 5.5× [5.1, 5.8] |
| 2 000 | **5.6×** [5.2, 6.3] | 2.4× [2.3, 2.5] |
| 20 000 | **4.2×** [4.1, 4.2] | 1.0× [1.0, 1.0] |

---

## 6. Reading the numbers

**Endpoint independence (Track B) holds — and the wasm2c→clang path is free.**
For pure compute at 100 k files, `wasm2c → clang` is statistically **tied with
native Rust** (0.99×, CI [0.97, 1.03]): translating wasm back to C and letting
clang optimise recovers all of the native performance. The cosmocc **universal
binary** costs only ~5–6 % and has the *smallest* memory footprint of any
endpoint (0.4 MiB cold, 48 MiB at 100 k). `wasmtime` is the outlier: ~24 %
slower at 100 k and a ~4 ms cold-start (module JIT) with an ~18 MiB runtime
floor — the price of a general-purpose sandbox.

**The Rust port vs Python (Track A) — biggest wins are startup and small jobs.**
Native Rust starts **~50× faster** than Python (0.5 ms vs 27 ms — no interpreter,
no imports) and organizes 2 000 files ~5.6× faster. At 20 000 files the gap
narrows to **4.2×**: the job becomes dominated by kernel `rename`/`stat`
syscalls, which cost the same in any language. Memory is ~10× leaner for small
jobs and converges at scale (the path working-set dominates).

**wasm32-wasip1 under wasmtime pays a filesystem tax.** As a *pure* core it is
only ~24 % off native (Track B), but doing *real* file I/O (Track A) it drops to
Python-parity at 20 000 files (1.0×): every `readdir`/`rename`/`open` crosses the
WASI boundary into the host. wasm is competitive for compute, not for
syscall-bound work.

---

## 7. What this says about the BDD workflow

* **The feature files were a portable contract, and a good one.** The same nine
  `.feature` files drove a Python implementation (pytest-bdd) and an independent
  Rust one (gherkin-cargo-test) to **byte-identical** behavior. That is the
  N-version verification the framework is built around, realised end-to-end.
* **A strict runner earns its keep.** `gherkin-cargo-test` immediately caught a
  latent grammar ambiguity (`collisions.feature`) that pytest-bdd tolerated, and
  its "unbound step ⇒ fail" ratchet meant getting to green *was* the equivalence
  proof — there was no silent gap to hide in.
* **Writing the contract down first exposed undocumented behavior.** Turning the
  five probed behaviors into `edge_behaviors.feature` (Set B) makes the port's
  contract *stronger* than the Python suite it came from.
* **"Compiler/endpoint independence" is real for a pure core.** Factor the logic
  into an fs-free `core`, and one source compiles to native, WASI, and
  wasm2c-via-C without change, producing identical output. The endpoint then
  becomes a deployment choice (native speed / sandboxed / single universal
  binary), not a correctness variable.

---

## 8. Reproducibility

```sh
# from repo root
cd rust
./build_endpoints.sh                 # builds all four endpoints (+ assimilated ELF)
cargo test --test features_original  # Set A  -> 68 passed
cargo test --test features_augmented # Set B  -> 75 passed
python3 gen_listing.py 100000 1 0 > bench_build/inputs/listing_100000.txt
python3 bench_build/run_bench.py 25  # -> bench_build/results.json
python3 gen_report.py                # -> tables

# Python↔Rust differential
bash diff_check.sh          # ALL DIFFERENTIAL CHECKS IDENTICAL
```

Toolchains used are local: `cargo` 1.97, `clang` 21, `cosmocc` 14.1 (from
`cosmo.zip`), `wasmtime` 46, `wasm2c` 1.0.41 (WABT), `gherkin-cargo-test` 0.2.

---

## 9. Caveats / threats to validity

* Single machine (AMD Ryzen 5 PRO 8500GE, 12 threads, Linux), work trees on
  `/dev/shm` to isolate CPU/startup from disk. Absolute ms are machine-specific;
  the *ratios* are the result.
* Track A moves real files but on tmpfs; on spinning/networked storage the
  syscall-bound regime (where languages converge) would dominate even sooner.
* The Python "cold start" is a console-script entry point on CPython 3.13; a
  long-running server amortises it away (not this tool's usage model).
* wasm2c endpoints benchmark the **pure planning core**, by design — the wasm
  never touches the filesystem. Real file moves are measured only where they are
  native/WASI (Track A). The two tracks answer two different questions and should
  not be compared cell-to-cell.
* `wasmtime run` JITs each invocation; an AOT-compiled (`wasmtime compile`)
  module would cut its cold-start but not its steady-state or FS-syscall cost.
