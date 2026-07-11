#!/usr/bin/env bash
# N-version differential: run the Python and Rust CLIs on byte-identical
# workspaces and diff their reports (normalizing only the absolute-path header)
# and their resulting file trees.
set -u
PY=/home/biho/gitrepos/file_organizer/.venv/bin/file-organizer
RS=/home/biho/gitrepos/file_organizer/rust/target/debug/file-organizer
ROOT=$(mktemp -d)
FAIL=0

norm() { sed -E 's#^(Organizing|Undoing last run in): .*#\1: <PATH>#'; }
tree_of() { (cd "$1" && find . -type f -o -type d | LC_ALL=C sort); }

# Populate a workspace given a scenario name.
build() {
  local d="$1" s="$2"
  mkdir -p "$d"
  case "$s" in
    classify)
      for f in run01.storx run02.storx sample.mzML calib.DMT notes.txt README.md archive.tar.gz Makefile .gitignore weird. a.b.; do
        echo "content-of-$f" > "$d/$f"; done ;;
    collide)
      mkdir -p "$d/TXT_Files"; echo first > "$d/TXT_Files/report.txt"; echo second > "$d/TXT_Files/report_1.txt"
      echo third > "$d/report.txt"; echo x > "$d/notes.txt" ;;
    recursive)
      mkdir -p "$d/a/b" "$d/data"; echo x > "$d/top.md"; echo y > "$d/data/notes.txt"; echo z > "$d/a/b/deep.storx"; echo w > "$d/report.txt"; mkdir -p "$d/sub"; echo n > "$d/sub/report.txt" ;;
    keep)
      mkdir -p "$d/batch1" "$d/batch2" "$d/a/b"; echo a > "$d/batch1/a.stori"; echo b > "$d/batch1/notes.txt"; echo c > "$d/batch2/c.stori"; echo d > "$d/a/b/c.stori" ;;
    empty) : ;;
  esac
}

run_case() {
  local name="$1" scen="$2"; shift 2
  local pd="$ROOT/$name/py" rd="$ROOT/$name/rs"
  build "$pd" "$scen"; build "$rd" "$scen"
  po=$("$PY" "$pd" "$@" 2>&1); pc=$?
  ro=$("$RS" "$rd" "$@" 2>&1); rc=$?
  local ok=1
  if [ "$pc" != "$rc" ]; then echo "  [$name] EXIT differs: py=$pc rs=$rc"; ok=0; fi
  if ! diff <(echo "$po"|norm) <(echo "$ro"|norm) >/dev/null; then
     echo "  [$name] REPORT differs:"; diff <(echo "$po"|norm) <(echo "$ro"|norm) | head -20; ok=0; fi
  if ! diff <(tree_of "$pd") <(tree_of "$rd") >/dev/null; then
     echo "  [$name] TREE differs:"; diff <(tree_of "$pd") <(tree_of "$rd") | head -20; ok=0; fi
  if [ "$ok" = 1 ]; then echo "  [$name] identical (exit=$rc)"; else FAIL=1; fi
}

echo "== organize =="
run_case classify_org classify
run_case collide_org collide
run_case recursive_org recursive --recursive
run_case keep_org keep --recursive --keep-structure
run_case empty_org empty
echo "== dry-run =="
run_case classify_dry classify --dry-run
run_case keep_dry keep --recursive --keep-structure --dry-run

# Undo scenarios: organize first (both), then undo both, diff the undo report+tree.
echo "== undo (organize then undo) =="
undo_case() {
  local name="$1" scen="$2"; shift 2
  local pd="$ROOT/$name/py" rd="$ROOT/$name/rs"
  build "$pd" "$scen"; build "$rd" "$scen"
  "$PY" "$pd" "$@" >/dev/null 2>&1; "$RS" "$rd" "$@" >/dev/null 2>&1
  po=$("$PY" "$pd" --undo 2>&1); pc=$?
  ro=$("$RS" "$rd" --undo 2>&1); rc=$?
  local ok=1
  [ "$pc" != "$rc" ] && { echo "  [$name] undo EXIT differs py=$pc rs=$rc"; ok=0; }
  diff <(echo "$po"|norm) <(echo "$ro"|norm) >/dev/null || { echo "  [$name] undo REPORT differs:"; diff <(echo "$po"|norm) <(echo "$ro"|norm)|head; ok=0; }
  diff <(tree_of "$pd") <(tree_of "$rd") >/dev/null || { echo "  [$name] undo TREE differs:"; diff <(tree_of "$pd") <(tree_of "$rd")|head; ok=0; }
  [ "$ok" = 1 ] && echo "  [$name] undo identical (exit=$rc)" || FAIL=1
}
undo_case classify_undo classify
undo_case recursive_undo recursive --recursive
undo_case keep_undo keep --recursive --keep-structure

rm -rf "$ROOT"
echo "======================================"
[ "$FAIL" = 0 ] && echo "ALL DIFFERENTIAL CHECKS IDENTICAL" || echo "SOME DIFFERENCES FOUND"
exit $FAIL