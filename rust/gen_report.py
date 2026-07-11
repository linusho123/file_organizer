#!/usr/bin/env python3
"""Render bench_build/results.json + env.txt into REPORT.md tables."""
import json
from pathlib import Path

RUST = Path(__file__).resolve().parent
BB = RUST / "bench_build"
res = json.loads((BB / "results.json").read_text())
env = (BB / "env.txt").read_text().strip()


def ms(x):
    if x is None:
        return "—"
    v = x * 1000.0
    if v >= 100:
        return f"{v:.0f}"
    if v >= 10:
        return f"{v:.1f}"
    return f"{v:.2f}"


def kib(x):
    return "—" if not x else f"{x/1024:.1f}"


def row(cells):
    return "| " + " | ".join(cells) + " |"


out = []
w = out.append

w("## Benchmark data\n")
w("```")
w(env)
w(f"benchmark reps: {res['reps']} (100k workloads use fewer; medians reported)")
w("```\n")

# binary sizes
w("### Artifact sizes\n")
w(row(["Artifact", "Size (KiB)"]))
w(row(["---", "---:"]))
for k, v in res["bin_sizes"].items():
    w(row([k, kib(v)]))
w("")

# ---- Track B ----
w("### Track B — pure planning core (stdin→stdout, no filesystem)\n")
w("The *same* Rust core compiled four ways. Byte-identical output at every size "
  "(md5 verified). Median wall-clock in **ms**; peak RSS in **MiB**.\n")
tb = res["track_b"]
eps_b = list(next(iter(tb.values())).keys())
# speed table
w("**Wall clock (median ms)**\n")
w(row(["workload"] + eps_b))
w(row(["---"] + ["---:"] * len(eps_b)))
for wl, d in tb.items():
    w(row([wl] + [ms(d[e]["median"]) for e in eps_b]))
w("")
w("**Peak memory (MiB, max-RSS)**\n")
w(row(["workload"] + eps_b))
w(row(["---"] + ["---:"] * len(eps_b)))
for wl, d in tb.items():
    w(row([wl] + [kib(d[e]["max_rss_kb"]) for e in eps_b]))
w("")
w("**Slowdown vs native at 100k** (median ratio, 95% bootstrap CI)\n")
w(row(["endpoint", "×slower", "CI95"]))
w(row(["---", "---:", "---"]))
for e in eps_b:
    s = tb["100k"][e]["slowdown_vs_native"]
    w(row([e, f"{s['median_ratio']:.2f}", f"[{s['ci95'][0]:.2f}, {s['ci95'][1]:.2f}]"]))
w("")

# ---- Track A ----
w("### Track A — real end-to-end CLI (organizes a real temp tree, --recursive)\n")
w("Python vs the Rust port (native and wasm32-wasip1 under wasmtime), all doing "
  "genuine filesystem moves on `/dev/shm`. `N` = files organized. Median ms; RSS MiB.\n")
ta = res["track_a"]
eps_a = list(next(iter(ta.values())).keys())
w("**Wall clock (median ms)**\n")
w(row(["N (files)"] + eps_a))
w(row(["---"] + ["---:"] * len(eps_a)))
for n, d in ta.items():
    label = f"{n}" + (" (cold start)" if n == "1" else "")
    w(row([label] + [ms(d[e]["median"]) for e in eps_a]))
w("")
w("**Peak memory (MiB, max-RSS)**\n")
w(row(["N (files)"] + eps_a))
w(row(["---"] + ["---:"] * len(eps_a)))
for n, d in ta.items():
    w(row([n] + [kib(d[e]["max_rss_kb"]) for e in eps_a]))
w("")
w("**Speedup vs Python** (median ratio, 95% bootstrap CI)\n")
w(row(["N"] + [f"{e}" for e in eps_a if e != "python"]))
cols = [e for e in eps_a if e != "python"]
w(row(["---"] + ["---:"] * len(cols)))
for n, d in ta.items():
    cells = [n]
    for e in cols:
        sp = d[e]["speedup_vs_python"]
        cells.append(f"{sp['median_ratio']:.1f}× [{sp['ci95'][0]:.1f},{sp['ci95'][1]:.1f}]")
    w(row(cells))
w("")

(RUST / "REPORT_TABLES.md").write_text("\n".join(out))
print("wrote REPORT_TABLES.md")
print("\n".join(out))
