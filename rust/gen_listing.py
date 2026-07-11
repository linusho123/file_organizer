#!/usr/bin/env python3
"""Emit a deterministic synthetic directory listing in the bench stdin format:
    line 1:  "<recursive> <keep_structure>"
    line n:  "<K>\t<relative/posix/path>"   K in {F,D,L,O}

Deterministic (seeded), with several extensions, nested dirs, and repeated
basenames so the planner exercises classification, sorting, and collision
resolution. Usage: gen_listing.py N [recursive] [keep] > listing.txt
"""
import sys

N = int(sys.argv[1]) if len(sys.argv) > 1 else 10000
recursive = sys.argv[2] if len(sys.argv) > 2 else "1"
keep = sys.argv[3] if len(sys.argv) > 3 else "0"

EXTS = ["storx", "dmt", "mzml", "txt", "md", "storx", "csv", "json", "storx", "log"]
NAMES = ["run", "sample", "calib", "report", "notes", "data", "scan", "trace"]

# Simple LCG so it is reproducible without importing random's implementation.
state = 0x2545F4914F6CDD1D
def rnd():
    global state
    state = (state * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
    return (state >> 33) & 0x7FFFFFFF

lines = ["%s %s" % (recursive, keep)]
dirs = set()
for i in range(N):
    depth = rnd() % 4  # 0..3 nested levels
    parts = []
    for _ in range(depth):
        parts.append("dir%d" % (rnd() % 40))
    name = NAMES[rnd() % len(NAMES)]
    # Force many collisions: small numeric space on the basename.
    num = rnd() % (N // 4 + 1)
    ext = EXTS[rnd() % len(EXTS)]
    fname = "%s%02d.%s" % (name, num, ext)
    parts.append(fname)
    rel = "/".join(parts)
    # Emit parent dirs (as D) once each, so the snapshot is well-formed.
    acc = []
    for p in parts[:-1]:
        acc.append(p)
        d = "/".join(acc)
        if d not in dirs:
            dirs.add(d)
            lines.append("D\t" + d)
    lines.append("F\t" + rel)

sys.stdout.write("\n".join(lines) + "\n")
