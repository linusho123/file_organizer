# AGENTS.md — File Organizer

Guidance for agents (and humans) working in this repo. The `.feature` files are
the contract; everything below is about *how* the code is built to satisfy them.

## What this is

- **`src/`** — the original Python CLI (upstream). Standard library only.
- **`rust/`** — a Rust port of the same tool that ships as a **single universal
  binary** (one `file-organizer.com` that runs on Windows, macOS, Linux, BSD).
- The experiment: the *same* Gherkin feature files drive both implementations,
  and the Rust core compiles to several endpoints (native, wasmtime, wasm2c→clang,
  wasm2c→cosmocc/APE) with byte-identical behavior.

## Architecture: reactor + driver — read this before touching the universal build

The Rust port is split so that **all logic is verifiable and endpoint-portable**,
and the impure part is a thin, dumb shell:

- **`rust/reactor`** — the **reactor**. It is **pure**: no filesystem, no clock,
  no randomness. It takes a *snapshot* of the directory plus `argv`, and returns a
  list of **primitive ops** (`mkdir` / `move` / `rmdir` / `write` / `delete`) plus
  the exact stdout/stderr/exit code. **Every decision lives here** — argument
  parsing, classification, collisions, the report text, the manifest JSON, undo.
- **driver** — thin plumbing. It does exactly two things the reactor cannot:
  *snapshot* the directory, and *replay* the reactor's ops. It holds **zero**
  organizing logic. There are two drivers, and they behave identically:
  - `rust/cli` — the native Rust driver (`std::fs`).
  - `rust/ape/driver.c` — the C driver, compiled into the APE.
- **`rust/core`** — the pure algorithms the reactor calls (extension parsing,
  collision resolution, plan/undo planning, byte-identical report strings). No deps.

Endpoints are produced by compiling the reactor to WebAssembly and translating it
back to C with `wasm2c`, then compiling that with clang or cosmocc.

## The rule that prevents the wrong turn

> When the target is **wasm2c / clang / a cosmocc APE**: keep the **reactor+driver**
> split. The reactor compiles to **`wasm32-unknown-unknown`** (NOT `wasm32-wasip1`).
> **No WASI. No syscall host. The driver owns all I/O.** Do not reach for a WASI
> shim (`uvwasi`, a hand-rolled preview1 host) or a C/other-language reimplementation
> of the logic — those throw away the verification and the shared core.

Why: a pure `wasm32-unknown-unknown` reactor is trivial to run through `wasm2c`
(no imports to satisfy). The moment I/O leaks into the wasm, you're forced into
WASI, which is the expensive, fragile path. Keep the wasm pure; keep the I/O in C.

## Verification — non-negotiable

- The feature files (`rust/features_original`, `rust/features_augmented`) verify the
  **reactor**, via `gherkin-cargo-test`, by driving the CLI binary as a subprocess.
- **Run the suite against the *shipped* endpoint, not just the dev build.** The
  harness reads `FO_BIN`:
  ```sh
  cd rust
  FO_BIN="$PWD/bench_build/file-organizer.com" cargo test --test features_augmented
  ```
  A universal binary is **not done** until Set B is green against that exact file.
- **Keep HOW out of the feature files.** They encode behavior only, so they stay
  language- and endpoint-neutral (that's what let the same files drive TypeScript,
  Python, and Rust). Architecture notes belong here, not in the Gherkin.

## Build

- Native + BDD suites: `cd rust && cargo test`  → Set A 68, Set B 75.
- Universal binary: `rust/ape/build-ape.sh`  → `rust/bench_build/file-organizer.com`.
  Needs `wasm2c` (WABT) and `cosmocc`; paths are overridable via env (see the
  script). **CI builds and releases the binary; it is never committed** to git.

## Releasing the universal binary

The binary is produced by CI, never committed. To publish a version:

```sh
git tag universal-vX.Y.Z
git push origin universal-vX.Y.Z
```

That fires `.github/workflows/universal-binary.yml`, which builds the APE
(wasm2c + cosmocc), **runs Gherkin Set B against the built binary**, and — only
if it's green — publishes a GitHub release with `file-organizer.com` attached.
A manual run (Actions → *universal-binary* → *Run workflow*) uploads it as an
artifact instead. The workflow never runs on ordinary pushes.

## Layout

```
rust/core       pure algorithms (no deps)
rust/reactor    the reactor: snapshot+argv -> ops+report (pure; -> wasm2c)
rust/cli        native driver (also the binary gherkin-cargo-test drives)
rust/ape        C driver (driver.c) + build-ape.sh + vendored wasm-rt runtime
rust/bench      pure-planning workload for the endpoint benchmark
rust/features_* Set A (original contract) and Set B (+ documented edge behaviors)
```
