#!/bin/bash
# Chooses which groups of tools/check.sh a pull request needs, from the paths it changes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/ci-scope.sh [--base <ref>]   the paths changed since the merge base with <ref> (default origin/main)
#        tools/ci-scope.sh --files          the paths come from standard input, one per line
#        tools/ci-scope.sh --self-test      check the table below, the path dependencies and the coverage, then exit
#
# Standard output is one line for tools/check.sh --groups: a comma list such as `base,report`, or `all` for the full gate. Standard
# error says why, one line per changed path with the rule that matched it. `base` is always in the answer.
#
# How it decides: each path takes the FIRST rule of RULES whose pattern matches it (a shell glob in which `*` also matches `/`), and
# the answer is the union of the groups of those rules. A path that no rule matches selects `all`: a file the table has never heard
# of may change anything, so the safe answer is the full gate. A rule may name `all` itself (the workflow, the scripts that decide
# what runs). Then the closure: `compare` reads what `shelldos` and `native` leave in dist/, so selecting it selects them too. No
# changed paths at all selects `base`. This follows Fizz Level Studio's tools/ci-scope.sh.
#
# The groups are the ones tools/check.sh runs (--names). A new directory means a new rule here; until then its changes run the full
# gate and the self-test (the base group's "CI scope table" step) fails, naming the files no rule covers.
set -euo pipefail
cd "$(dirname "$0")/.."

ALL_GROUPS="base report fonts rust dos stdstubs shelldos native compare"

# pattern ; groups ; example path. The self-test checks each rule against its example, so an example must reach its own rule first.
# Order matters: a specific rule goes above the general one that would also match.
RULES=(
  # Records and static files: only the base checks read them.
  'LICENSE-*;base;LICENSE-MIT'
  'NOTICE;base;NOTICE'
  '.gitignore;base;.gitignore'
  '.gitattributes;base;.gitattributes'
  'assets/*;base;assets/hero.svg'
  'tools/build-hero.py;base;tools/build-hero.py'
  # Documentation, and the design boards the report generator draws pictures from: the manual's tests read them.
  '*.md;base,report;docs/SPEC.md'
  'docs/*;base,report;docs/images/first-window/dos-8-typed.png'
  'design/*;base,report;design/shell-og/screenshots/v4-01-desktop.png'
  # The workflow and the files that decide what runs or how it is set up: the full gate.
  '.github/workflows/*;all;.github/workflows/ci.yml'
  '.github/*;base;.github/dependabot.yml'
  'tools/check.sh;all;tools/check.sh'
  'tools/ci-scope.sh;all;tools/ci-scope.sh'
  'tools/setup.sh;all;tools/setup.sh'
  'tools/fetch-tools.sh;all;tools/fetch-tools.sh'
  'rust-toolchain.toml;all;rust-toolchain.toml'
  # The base checks themselves.
  'tools/check-headers.sh;base;tools/check-headers.sh'
  'tools/check-docs.py;base;tools/check-docs.py'
  # The progress report and the frame tools.
  'tools/progress-report/*;report;tools/progress-report/gather.py'
  'tools/test_frames.py;report;tools/test_frames.py'
  'tools/frames.py;report,shelldos,native,compare;tools/frames.py'
  'tools/native-inject.py;report,native,compare;tools/native-inject.py'
  # The fonts. The generated faces are compiled into the shell's tests, the kernel and SHELL.EXE.
  'fonts/generated/*;report,fonts,rust,shelldos,native,compare;fonts/generated/ui.dfn'
  'fonts/*;report,fonts;fonts/build.ts'
  # The shared crates. hal/ is built by every Rust workspace that runs; shell/ by its host tests, the kernel and SHELL.EXE.
  'hal/*;report,rust,dos,stdstubs,shelldos,native,compare;hal/drivers/kbd/src/lib.rs'
  'shell/dos/*;report,shelldos,compare;shell/dos/src/main.rs'
  'shell/*;report,rust,shelldos,native,compare;shell/gfx/src/lib.rs'
  # The hosted track's skeleton and its build tools.
  'core/*;report,rust,dos;core/src/lib.rs'
  'c/*;report,dos;c/main.c'
  'tools/build.sh;report,dos;tools/build.sh'
  'tools/coff2go32.py;report,dos;tools/coff2go32.py'
  'tools/raw2png.py;report,dos;tools/raw2png.py'
  'tools/dosrun.sh;report,dos,stdstubs,shelldos,compare;tools/dosrun.sh'
  'tools/elf2go32.py;report,dos,stdstubs,shelldos,compare;tools/elf2go32.py'
  # The spikes. The pure Rust DOS runtime and its target are shared by the shell on DOS, the other spikes and the cross checks.
  'spikes/d0-rust-dos/rt/*;report,rust,dos,stdstubs,shelldos,compare;spikes/d0-rust-dos/rt/src/lib.rs'
  'spikes/d0-rust-dos/dos-elf.json;report,rust,dos,shelldos,compare;spikes/d0-rust-dos/dos-elf.json'
  'spikes/d0-rust-dos/link.ld;report,rust,dos,shelldos,compare;spikes/d0-rust-dos/link.ld'
  'spikes/d0-rust-dos/*;report,dos;spikes/d0-rust-dos/progs/src/hello.rs'
  # The kernel compiles spike A's scene drawing in (a #[path] module), so the native test redraws it.
  'spikes/a-vga-vbe/prog/src/scene.rs;report,dos,native,compare;spikes/a-vga-vbe/prog/src/scene.rs'
  'spikes/a-vga-vbe/*;report,dos;spikes/a-vga-vbe/check.sh'
  'spikes/b-dos-input/expected/*;report,rust,dos;spikes/b-dos-input/expected/keys.txt'
  'spikes/b-dos-input/*;report,dos;spikes/b-dos-input/check.sh'
  'spikes/e-std-stubs/*;report,stdstubs;spikes/e-std-stubs/check.sh'
  'spikes/s1-dxe3/*;report,rust,dos;spikes/s1-dxe3/run.sh'
  'spikes/s5-jsdos/*;report,rust;spikes/s5-jsdos/probes/Cargo.toml'
  # The native track.
  'kernel/i486-desk-none.json;report,rust,native,compare;kernel/i486-desk-none.json'
  'kernel/*;report,native,compare;kernel/src/main.rs'
  'boot/*;report,native,compare;boot/stage2.asm'
  'tools/native-test-bochs.sh;report;tools/native-test-bochs.sh'
  'tools/native-run-bochs.sh;report;tools/native-run-bochs.sh'
  'tools/native-test-86box.sh;report;tools/native-test-86box.sh'
  'tools/native-run-86box.sh;report;tools/native-run-86box.sh'
  'tools/native-*;report,native,compare;tools/native-run.sh'
  'tools/elf2bin.py;report,native,compare;tools/elf2bin.py'
  # The goldens and transcripts.
  'tests/shell/*;report,rust,shelldos,native,compare;tests/shell/first-window.sums'
  'tests/native/*;report,native,compare;tests/native/expected.txt'
  'tools/shell-compare.sh;report,compare;tools/shell-compare.sh'
  # Opt-in runs and tools no check runs; the manual's tests read some of them.
  'tools/shell-goldens.sh;report;tools/shell-goldens.sh'
  'tools/run-shell.sh;report;tools/run-shell.sh'
  'tools/xvfb-dos.sh;report;tools/xvfb-dos.sh'
  'tools/check-jsdos.sh;report;tools/check-jsdos.sh'
  'tools/docker/*;report;tools/docker/Dockerfile.native-bochs'
  'tools/86box/*;report;tools/86box/86box.cfg'
  'tools/desk.magic;report;tools/desk.magic'
  'tools/build-*;report;tools/build-runtime-presentation.py'
)

# rule_of PATH: prints "pattern;groups" of the first rule that matches, or nothing
rule_of() {
  local rule pat
  for rule in "${RULES[@]}"; do
    pat="${rule%%;*}"
    # shellcheck disable=SC2254
    case "$1" in $pat) printf '%s\n' "${rule%;*}"; return ;; esac
  done
}

# order LIST: the comma list in the canonical group order, without repeats, with the closure applied (compare needs shelldos, native)
order() {
  local out='' g list=",$1,"
  case "$list" in *,compare,*) list="$list,shelldos,native," ;; esac
  for g in $ALL_GROUPS; do
    case "$list" in *",$g,"*) out="$out${out:+,}$g" ;; esac
  done
  echo "$out"
}

# classify: paths on standard input; the answer on standard output, the reasons on standard error
classify() {
  local f r any=0 all=0 sel=base
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    any=1
    r="$(rule_of "$f")"
    if [ -z "$r" ]; then
      echo "ci-scope: $f -> all (no rule matches this path)" >&2
      all=1
      continue
    fi
    echo "ci-scope: $f -> ${r#*;} (rule '${r%%;*}')" >&2
    if [ "${r#*;}" = all ]; then all=1; else sel="$sel,${r#*;}"; fi
  done
  [ "$any" = 1 ] || echo "ci-scope: no changed paths -> base" >&2
  if [ "$all" = 1 ]; then echo all; else order "$sel"; fi
}

fail() { echo "ci-scope: FAIL: $*" >&2; exit 1; }

# covers SUPER SUB: every group of the comma list SUB is in SUPER (`all` covers everything)
covers() {
  local g
  [ "$1" = all ] && return 0
  [ "$2" = all ] && return 1
  for g in ${2//,/ }; do case ",$1," in *",$g,"*) ;; *) return 1 ;; esac; done
}

# groups_of PATH: the answer for that one path
groups_of() { printf '%s\n' "$1" | classify 2> /dev/null; }

self_test() {
  local rule pat groups example want got n=0 bad=0
  # One test per rule: its example path, alone, gives exactly the rule's groups (with base and the closure).
  for rule in "${RULES[@]}"; do
    pat="${rule%%;*}"
    groups="${rule#*;}"; example="${groups#*;}"; groups="${groups%%;*}"
    if [ "$groups" = all ]; then want=all; else want="$(order "base,$groups")"; fi
    got="$(groups_of "$example")"
    [ "$got" = "$want" ] || { echo "ci-scope: rule '$pat': example $example gave '$got', expected '$want'" >&2; bad=1; }
    git ls-files --error-unmatch -- "$example" > /dev/null 2>&1 || { echo "ci-scope: note: the example of rule '$pat' ($example) is not a tracked file" >&2; }
    n=$((n + 1))
  done
  # Every group a rule names exists.
  for rule in "${RULES[@]}"; do
    groups="${rule#*;}"; groups="${groups%%;*}"
    for g in ${groups//,/ }; do
      case " all $ALL_GROUPS " in *" $g "*) ;; *) echo "ci-scope: rule '${rule%%;*}' names the unknown group '$g'" >&2; bad=1 ;; esac
    done
  done
  # The groups are the ones check.sh runs, in its order.
  got="$(tools/check.sh --names | tr '\n' ' ' | sed 's/ $//')"
  [ "$got" = "$ALL_GROUPS" ] || { echo "ci-scope: tools/check.sh --names is '$got', not '$ALL_GROUPS'" >&2; bad=1; }
  # The combinations the rules exist for.
  expect() {
    local want="$1" got
    shift
    got="$(printf '%s\n' "$@" | classify 2> /dev/null)"
    [ "$got" = "$want" ] || { echo "ci-scope: [$*] gave '$got', expected '$want'" >&2; bad=1; }
    n=$((n + 1))
  }
  expect base
  expect base,report README.md docs/SPEC.md tools/progress-report/manual/M21-evidence.md
  expect base,report tools/progress-report/gather.py
  expect all some/new/file.rs
  expect all docs/SPEC.md some/new/file.rs
  expect all .github/workflows/ci.yml
  expect all tools/check.sh
  expect all rust-toolchain.toml
  expect base,report,rust,dos,stdstubs,shelldos,native,compare hal/hal/src/lib.rs
  expect base,report,rust,shelldos,native,compare shell/gfx/src/lib.rs
  expect base,report,shelldos,native,compare tools/shell-compare.sh
  expect base,report,shelldos,native,compare kernel/src/main.rs boot/stage1.asm
  expect base,report,fonts fonts/src/emoji/README.txt
  expect base,report,stdstubs spikes/e-std-stubs/shim/src/lib.rs
  expect base LICENSE-MIT assets/hero.svg
  expect all 'docs/a b.md' 'x y'
  # The path dependencies: a change in a directory something depends on must select every group the dependent selects.
  closure || bad=1
  # Coverage: every tracked file reaches a rule (a file that falls to `all` means a missing rule).
  local f missing=0
  while IFS= read -r f; do
    [ -n "$(rule_of "$f")" ] || { echo "ci-scope: no rule covers the tracked file $f (it selects all)" >&2; missing=$((missing + 1)); }
  done < <(git ls-files)
  [ "$missing" = 0 ] || { echo "ci-scope: $missing tracked files fall to all; add rules for them" >&2; bad=1; }
  [ "$bad" = 0 ] || fail "the self-test found problems (above)"
  echo "ci-scope: self-test ok ($n tests, $(git ls-files | wc -l) tracked files covered)"
}

# norm DIR REL: REL resolved against DIR, relative to the repository root
norm() { realpath -m --relative-to=. "$1/$2"; }

# need DEP_PATH WANT WHY: a change at DEP_PATH must select the groups WANT
need() {
  local got
  got="$(groups_of "$1")"
  covers "$got" "$2" || { echo "ci-scope: $3: $1 selects '$got', which lacks some of '$2'" >&2; return 1; }
}

closure() {
  local bad=0 m dir dep line f rel w g block tests=0
  # 1. Cargo path dependencies: [dependencies] entries with `path = "..."` that name a directory with a Cargo.toml.
  while IFS= read -r m; do
    dir="$(dirname "$m")"
    while IFS= read -r rel; do
      dep="$(norm "$dir" "$rel")"
      [ -f "$dep/Cargo.toml" ] || continue
      need "$dep/src/lib.rs" "$(groups_of "$dir/src/lib.rs")" "$m depends on $dep" || bad=1
      tests=$((tests + 1))
    done < <(sed -n 's/.*path *= *"\([^"]*\)".*/\1/p' "$m")
  done < <(git ls-files '*Cargo.toml')
  # 2. Cargo configuration: the target specification and the linker script a workspace builds with.
  while IFS= read -r m; do
    w="$(dirname "$(dirname "$m")")"
    while IFS= read -r rel; do
      dep="$(norm "$w" "$rel")"
      git ls-files --error-unmatch -- "$dep" > /dev/null 2>&1 || continue
      need "$dep" "$(groups_of "$w/src/lib.rs")" "$m names $dep" || bad=1
      tests=$((tests + 1))
    done < <(sed -n -e 's/^target *= *"\([^"]*\)".*/\1/p' -e 's/.*-T\([^", ]*\).*/\1/p' "$m")
  done < <(git ls-files '*/.cargo/config.toml')
  # 3. Files compiled in with include_bytes! and include_str!. Test code needs only the rust group, which runs the host tests.
  while IFS= read -r line; do
    f="${line%%:*}"; rel="${line#*:}"
    dep="$(norm "$(dirname "$f")" "$rel")"
    case "$dep" in dist/*) continue ;; esac
    case "$f" in */tests/* | */tests.rs) g=rust ;; *) g="$(groups_of "$f")" ;; esac
    need "$dep" "$g" "$f includes $dep" || bad=1
    tests=$((tests + 1))
  done < <(git grep -oE 'include_(bytes|str)!\("[^"]+"\)' -- '*.rs' | sed -E 's/^([^:]+):include_(bytes|str)!\("([^"]+)"\)$/\1:\3/')
  # 4. Source files pulled in with #[path = "..."]: the module belongs to the crate that names it.
  while IFS= read -r line; do
    f="${line%%:*}"; rel="${line#*:}"
    dep="$(norm "$(dirname "$f")" "$rel")"
    git ls-files --error-unmatch -- "$dep" > /dev/null 2>&1 || continue
    need "$dep" "$(groups_of "$f")" "$f names the module $dep" || bad=1
    tests=$((tests + 1))
  done < <(git grep -E '^#\[path *= *"[^"]+"\]' -- '*.rs' | sed -E 's/^([^:]+):#\[path *= *"([^"]+)"\].*$/\1:\2/')
  # 5. The cross builds in tools/check.sh: `(cd W && ... --target ../X ...)` inside an `if run GROUP ...` block needs GROUP.
  block=
  while IFS= read -r line; do
    case "$line" in 'if run '*) block="${line#if run }"; block="${block%%[ ;]*}" ;; fi) block= ;; esac
    if [[ "$line" =~ \(cd\ ([^ ]+)\ \&\&.*--target\ ([^ ]+) ]]; then
      dep="$(norm "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}")"
      [ -n "$block" ] || { echo "ci-scope: tools/check.sh: a cross build outside a group block: $line" >&2; bad=1; continue; }
      need "$dep" "$block" "tools/check.sh's $block group builds with $dep" || bad=1
      tests=$((tests + 1))
    fi
  done < tools/check.sh
  echo "ci-scope: path dependencies: $tests edges checked" >&2
  return "$bad"
}

# changed_since REF: the paths changed on this branch since its merge base with REF (renames count as a delete and an add)
changed_since() {
  git rev-parse --verify --quiet "$1^{commit}" > /dev/null || fail "cannot resolve '$1' (fetch it, with enough history for a merge base)"
  git diff --name-only --no-renames "$1...HEAD" || fail "cannot diff against '$1'"
}

case "${1:-}" in
  --self-test) self_test ;;
  --files) classify ;;
  --base) [ $# -ge 2 ] || fail "--base needs a ref"; paths="$(changed_since "$2")"; classify <<< "$paths" ;;
  "") paths="$(changed_since origin/main)"; classify <<< "$paths" ;;
  *) fail "unknown argument $1 (see the usage at the top of the script)" ;;
esac
