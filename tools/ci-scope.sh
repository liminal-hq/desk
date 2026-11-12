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
# Standard output is one line for tools/check.sh --groups: a comma list such as `base,report`. The full gate is every group name
# spelled out (`all` is expanded here), so a workflow can test a group with contains(format(',{0},', groups), ',rust,') and need no
# special case for `all`. Standard error says why, one line per changed path with the rule that matched it. `base` is always in the
# answer.
#
# How it decides: each path takes the FIRST rule of RULES whose pattern matches it (a shell glob in which `*` also matches `/`), and
# the answer is the union of the groups of those rules. A path that no rule matches selects `all`: a file the table has never heard
# of may change anything, so the safe answer is the full gate. A rule may name `all` itself (the workflow, the scripts that decide
# what runs). Then the closure: `compare` reads what `shelldos` and `native` leave in dist/, so selecting it selects them too. No
# changed paths at all selects `base`. This follows Fizz Level Studio's tools/ci-scope.sh.
#
# The groups are the ones tools/check.sh runs (--names). A new directory means a new rule here; until then its changes run the full
# gate and the self-test (the base group's "CI scope table" step) fails, naming the files no rule covers.
#
# What the self-test cannot see: it reads the files a group's steps name (the scripts, `(cd DIR` directories, and the tracked files
# those scripts and the Bun and Rust tests mention as string literals or through include_bytes!, include_str!, include! and #[path]),
# so a file a program computes the name of, or reads through a path assembled at run time, is a dependency only a person can add. For
# those, and for a script that calls a script that calls a file, put the path in a rule by hand.
set -euo pipefail
SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
cd "$(dirname "$0")/.."

# The script steps_closure parses; the self-test points it at mutated copies to prove the parser notices what it drops.
CHECK_SH="${CI_SCOPE_CHECK_SH:-tools/check.sh}"

ALL_GROUPS="base report fonts rust dos stdstubs shelldos native compare"

# Paths with non-ASCII characters come out of git unquoted, so they match the rules as they are written.
git() { command git -c core.quotePath=false "$@"; }

# pattern ; groups ; example path. The self-test checks each rule against its example, so an example must reach its own rule first.
# Order matters: a specific rule goes above the general one that would also match.
RULES=(
  # Records and static files: only the base checks read them.
  'LICENSE-*;base;LICENSE-MIT'
  'NOTICE;base;NOTICE'
  '.gitignore;base;.gitignore'
  '.gitattributes;all;.gitattributes'
  'assets/*;base;assets/hero.svg'
  # The progress report's tests walk every .py, .sh and .rs file, so the tools the base group runs are report inputs too.
  'tools/build-hero.py;base,report;tools/build-hero.py'
  # Documentation, and the design boards the report generator draws pictures from: the manual's tests read them. The palette and the
  # emoji roadmap are also read by the font tests (fonts/test/emoji-pixel.test.ts, fonts/test/emoji-roadmap.test.ts).
  'docs/PALETTE.md;base,report,fonts;docs/PALETTE.md'
  'docs/EMOJI-ROADMAP.md;base,report,fonts;docs/EMOJI-ROADMAP.md'
  '*.md;base,report;docs/SPEC.md'
  'docs/*;base,report;docs/images/first-window/dos-8-typed.png'
  'design/*;base,report;design/shell-og/screenshots/v4-01-desktop.png'
  # The workflow and the files that decide what runs or how it is set up: the full gate.
  '.github/workflows/*;all;.github/workflows/ci.yml'
  '.github/actions/*;all;.github/actions/setup/action.yml'
  '.github/*;base;.github/dependabot.yml'
  'tools/check.sh;all;tools/check.sh'
  'tools/ci-scope.sh;all;tools/ci-scope.sh'
  'tools/setup.sh;all;tools/setup.sh'
  'tools/fetch-tools.sh;all;tools/fetch-tools.sh'
  'rust-toolchain.toml;all;rust-toolchain.toml'
  # The base checks themselves (and report inputs, as above).
  'tools/check-headers.sh;base,report;tools/check-headers.sh'
  'tools/check-docs.py;base,report;tools/check-docs.py'
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
  # The module loader's host crates (the format reader and the packer, whose real-linker test builds a module for the kernel's target
  # specification, a path the kernel rules above already send to rust). module/mods and module/dos are excluded from the workspace and
  # will be built for the native and DOS tracks: give them rules of their own when those directories exist.
  'module/*;report,rust;module/format/src/lib.rs'
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
  expect base,report,rust module/pack/tests/real.rs module/Cargo.lock
  expect base,report,shelldos,native,compare tools/shell-compare.sh
  expect base,report,shelldos,native,compare kernel/src/main.rs boot/stage1.asm
  expect base,report,fonts fonts/src/emoji/README.txt
  expect base,report,stdstubs spikes/e-std-stubs/shim/src/lib.rs
  expect base LICENSE-MIT assets/hero.svg
  expect all 'docs/a b.md' 'x y'
  expect base,report 'docs/Ünïcode ✓.md'
  expect base,report,fonts docs/PALETTE.md docs/EMOJI-ROADMAP.md
  expect all .gitattributes .github/actions/setup/action.yml
  expect base,report tools/check-headers.sh tools/check-docs.py tools/build-hero.py
  # `all` is spelled out on standard output, so contains(',rust,') works on it.
  got="$(printf 'some/new/file.rs\n' | bash "$SELF" --files 2> /dev/null)" || got="(failed)"
  [ "$got" = "${ALL_GROUPS// /,}" ] || { echo "ci-scope: --files for an unknown path gave '$got', not every group" >&2; bad=1; }
  n=$((n + 1))
  # Paths with non-ASCII characters come out of git unquoted.
  scratch_repo_test || bad=1
  n=$((n + 1))
  PT=0
  # The path dependencies: a change in a directory something depends on must select every group the dependent selects.
  closure || bad=1
  # The files each group's steps read: the scripts and directories its steps in tools/check.sh name, what those scripts and the font
  # and Rust tests read.
  steps_closure || bad=1
  # The parser of tools/check.sh must notice what it cannot read: each mutated copy fails, a harmless one passes.
  parser_tests || bad=1
  n=$((n + PT))
  # Coverage: every tracked file reaches a rule (a file that falls to `all` means a missing rule).
  local f missing=0
  while IFS= read -r f; do
    [ -n "$(rule_of "$f")" ] || { echo "ci-scope: no rule covers the tracked file $f (it selects all)" >&2; missing=$((missing + 1)); }
  done < <(git ls-files)
  [ "$missing" = 0 ] || { echo "ci-scope: $missing tracked files fall to all; add rules for them" >&2; bad=1; }
  [ "$bad" = 0 ] || fail "the self-test found problems (above)"
  echo "ci-scope: self-test ok ($n tests, $(git ls-files | wc -l) tracked files covered)"
}

# parser_tests: steps_closure on mutated copies of tools/check.sh. Extra spaces and a one-line `if ...; fi` must still read as before;
# a deleted block, a stray or missing `fi` and a block name the parser cannot read must fail
parser_tests() {
  local dir m ok=0 want
  dir="$(mktemp -d)"
  # name|sed script|expected: fail or pass
  local -a muts=(
    'double space after if|s/^if run native; then/if  run native; then/|pass'
    'a block name the parser cannot read|s/^if run native; then/if run 9native; then/|fail'
    'deleted native block|/^if run native; then/,/^fi$/d|fail'
    'stray fi before native|s/^if run native; then/fi\nif run native; then/|fail'
    'missing fi in the native block|/^tools\/native-test.sh$/{n;d}|fail'
    'one-line if inside a block|s/^step "Native boot: NASM.*/&\nif true; then :; fi/|pass'
    'extra spaces in if run|s/^if run compare; then/if   run   compare; then/|pass'
  )
  for m in "${muts[@]}"; do
    IFS='|' read -r name script want <<< "$m"
    sed -E "$script" tools/check.sh > "$dir/check.sh"
    if cmp -s tools/check.sh "$dir/check.sh"; then echo "ci-scope: parser test '$name' changed nothing" >&2; ok=1; continue; fi
    if CI_SCOPE_CHECK_SH="$dir/check.sh" bash "$SELF" --steps > /dev/null 2>&1; then got=pass; else got=fail; fi
    case "$want" in
      "$got") ;;
      *) echo "ci-scope: parser test '$name': the closure should $want on the mutated tools/check.sh and did $got" >&2; ok=1 ;;
    esac
    PT=$((PT + 1))
  done
  rm -rf "$dir"
  return "$ok"
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
  done < <(git grep -E '^[[:space:]]*#\[path *= *"[^"]+"\]' -- '*.rs' | sed -E 's/^([^:]+):[[:space:]]*#\[path *= *"([^"]+)"\].*$/\1:\2/')
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

# scratch_repo_test: changed_since reports a non-ASCII path as it is written, not in git's quoted octal form
scratch_repo_test() {
  local dir got ok=0 root
  root="$(pwd)"
  dir="$(mktemp -d)"
  (
    cd "$dir"
    git init -q .
    git config user.name scope-test
    git config user.email scope-test@example.invalid
    git config commit.gpgsign false
    : > a.txt && git add a.txt && git commit -q -m one
    : > 'é ✓.md' && git add -- 'é ✓.md' && git commit -q -m two
    got="$(changed_since HEAD~1)"
    [ "$got" = 'é ✓.md' ] || { echo "ci-scope: git named the non-ASCII path '$got'" >&2; exit 1; }
  ) || ok=1
  cd "$root"
  rm -rf "$dir"
  return "$ok"
}

declare -A TRACKED=() GCACHE=() SEEN=() DOCS_BY_BASE=()

# groups_cached PATH: groups_of, remembered, left in GC (no subshell, so thousands of lookups stay cheap)
groups_cached() {
  [ -n "${GCACHE[$1]+x}" ] || GCACHE[$1]="$(groups_of "$1")"
  GC="${GCACHE[$1]}"
}

# normpath PATH: the path without `.` and `..` parts, left in NP, in pure bash (empty when it climbs out of the repository)
normpath() {
  local part out=() IFS=/
  NP=
  for part in $1; do
    case "$part" in
      '' | .) ;;
      ..) [ "${#out[@]}" -gt 0 ] || return 0; unset 'out[${#out[@]}-1]' ;;
      *) out+=("$part") ;;
    esac
  done
  NP="${out[*]}"
}

# require PATH GROUP WHY: PATH, when it is a tracked file, selects GROUP (each pair is checked once)
require() {
  [ -n "${TRACKED[$1]+x}" ] || return 0
  [ -z "${SEEN[$1|$2]+x}" ] || return 0
  SEEN[$1|$2]=1
  STEP_EDGES=$((STEP_EDGES + 1))
  groups_cached "$1"
  covers "$GC" "$2" || { echo "ci-scope: $3: $1 selects '$GC', which lacks '$2'" >&2; return 1; }
}

# scan_refs FILE GROUP QUOTED_ONLY BASES...: the tracked files FILE names, each resolved against the root and against each of the
# BASES directories, must select GROUP. Comment lines are skipped. QUOTED_ONLY=1 looks only at the string literals.
scan_refs() {
  local f="$1" g="$2" quoted="$3" tok base dep bad=0 pick='[A-Za-z0-9_./+-]+'
  shift 3
  [ "$quoted" = 1 ] && pick='"[^" ]+"'
  while IFS= read -r tok; do
    tok="${tok//\"/}"
    case "$tok" in */* | *.*) ;; *) continue ;; esac
    # A script's mentions of documents, boards and pictures are text for people, not inputs.
    # The Dockerfiles and the Xvfb runner are reached only by opt-in runs and fallbacks (INPUT_EXTRA, NATIVE_EXTRA, no local QEMU), and
    # are named in usage text; the CI image runs none of them.
    if [ "$quoted" = 0 ]; then
      case "$tok" in *.md | *.svg | *.png | docs/* | design/* | assets/* | tools/docker/* | tools/xvfb-dos.sh) continue ;; esac
    fi
    for base in "" "$@"; do
      normpath "${base:+$base/}$tok"; dep="$NP"
      [ -n "$dep" ] && [ "$dep" != "$f" ] || continue
      require "$dep" "$g" "$f (which the $g group runs) names $dep" || bad=1
      # A script that a script calls is read in turn (shell and Python only: a Rust or Bun file is a leaf).
      if [ "$quoted" = 0 ] && [ -n "${TRACKED[$dep]+x}" ]; then
        case "$dep" in *.sh | *.py) scan_script "$dep" "$g" || bad=1 ;; esac
      fi
    done
  done < <(grep -vE '^[[:space:]]*(#|//)' "$f" | grep -oE "$pick" | sort -u)
  return "$bad"
}

# scan_bun FILE: the font tests assemble repository paths from pieces (join(dir, '..', 'docs', 'PALETTE.md')), so a tracked document
# outside fonts/ whose name is a string literal of the file, and each directory of whose path is a string literal too (docs, PALETTE.md),
# is a file the fonts group reads; so is any literal path that resolves to a tracked file from the file's directory or from fonts/
scan_bun() {
  local f="$1" lit bad=0 cand part ok
  local -A lits=()
  while IFS= read -r lit; do lits[${lit:1:${#lit}-2}]=1; done < <(grep -oE "['\"\`][A-Za-z0-9_.+-]+['\"\`]" "$f" | sort -u)
  for lit in "${!lits[@]}"; do
    for cand in ${DOCS_BY_BASE[$lit]:-}; do
      case "$cand" in fonts/*) continue ;; esac
      ok=1
      for part in $(dirname "$cand" | tr / ' '); do [ "$part" = . ] || [ -n "${lits[$part]+x}" ] || ok=0; done
      [ "$ok" = 0 ] || require "$cand" fonts "$f reads $cand" || bad=1
    done
  done
  scan_refs "$f" fonts 1 "$(dirname "$f")" fonts || bad=1
  return "$bad"
}

# scan_script SCRIPT GROUP: what a script the group runs reads, and the scripts it calls in turn (each script once per group)
scan_script() {
  [ -n "${TRACKED[$1]+x}" ] || return 0
  # This script's table names every path it classifies; it reads none of them.
  [ "$1" != tools/ci-scope.sh ] || return 0
  [ -z "${SEEN[scan|$1|$2]+x}" ] || return 0
  SEEN[scan|$1|$2]=1
  scan_refs "$1" "$2" 0 "$(dirname "$1")"
}

# steps_closure: everything a group's steps in tools/check.sh run, and what that reads, selects that group
steps_closure() {
  local bad=0 line block='' depth=0 optin=0 f tok d first rel crate g
  STEP_EDGES=0
  TRACKED=()
  DOCS_BY_BASE=()
  while IFS= read -r f; do
    TRACKED[$f]=1
    case "$f" in *.md) DOCS_BY_BASE[${f##*/}]+="$f " ;; esac
  done < <(git ls-files)
  declare -A blocks=() reads=()
  # The text of each `if run GROUP` block, indented opt-in steps included; the opt-in blocks that rerun a whole group are not CI.
  # The nesting is counted from the `if` and `fi` keywords at any indentation (a one-line `if ...; fi` opens and closes at once), so a
  # block ends at its own `fi` and a nested `fi` does not end it early.
  local t delta prev optin_floor=0 parsed=0 loose
  while IFS= read -r line; do
    t="${line#"${line%%[![:space:]]*}"}"
    delta=0
    if [[ "$t" =~ ^if[[:space:]] ]]; then
      delta=1
      [[ "$t" =~ (^|[[:space:];])fi[[:space:]]*$ ]] && delta=0
    elif [[ "$t" =~ ^fi([[:space:];]|$) ]]; then
      delta=-1
    fi
    prev=$depth
    depth=$((depth + delta))
    [ "$depth" -ge 0 ] || { echo "ci-scope: tools/check.sh has a 'fi' with no 'if' before it: $line" >&2; bad=1; depth=0; }
    if [ "$prev" = 0 ]; then
      block=
      if [ "$delta" = 1 ] && [[ "$t" =~ ^if[[:space:]]+run[[:space:]]+([a-z]+) ]]; then block="${BASH_REMATCH[1]}"; parsed=$((parsed + 1)); fi
      continue
    fi
    # An `if [ -n "${..._EXTRA:-}" ]` is an opt-in run the CI does not make: its lines are skipped up to its own `fi`.
    if [ "$optin" = 1 ]; then [ "$depth" -le "$optin_floor" ] && optin=0; continue; fi
    if [ "$delta" = 1 ] && [[ "$t" =~ ^if[[:space:]].*_EXTRA ]]; then optin=1; optin_floor=$prev; continue; fi
    if [ "$depth" = 0 ]; then block=; continue; fi
    [ -z "$block" ] || blocks[$block]+="$line"$'\n'
  done < "$CHECK_SH"
  [ "$depth" = 0 ] || { echo "ci-scope: tools/check.sh ends inside an 'if' (depth $depth)" >&2; bad=1; }
  # The parser must see every `if run` block, and every group must have one that reads files: a block it silently dropped would
  # leave its group's steps unchecked.
  loose="$(grep -cE '^[[:space:]]*if[[:space:]]+run[[:space:]]' "$CHECK_SH" || true)"
  [ "$loose" = "$parsed" ] || { echo "ci-scope: tools/check.sh has $loose 'if run' lines but the parser read $parsed blocks" >&2; bad=1; }
  for g in $ALL_GROUPS; do
    [[ "${blocks[$g]:-}" =~ [^[:space:]] ]] || { echo "ci-scope: the parser found no 'if run $g' block with steps in tools/check.sh" >&2; bad=1; }
  done
  for g in "${!blocks[@]}"; do
    reads[$g]=$STEP_EDGES
    # Level 1: the tracked files named on a step's lines, and the directories of `(cd DIR && ...)`, with the files they run there.
    while IFS= read -r line; do
      [[ "$line" =~ ^[[:space:]]*# ]] && continue
      d=
      [[ "$line" =~ \(cd\ ([^\ \)]+) ]] && d="${BASH_REMATCH[1]}"
      first=1
      for tok in $(printf '%s\n' "$line" | grep -oE '[A-Za-z0-9_./+-]+'); do
        require "$tok" "$g" "tools/check.sh's $g group runs $tok" || bad=1
        if [ -n "$d" ]; then
          [ "$first" = 1 ] && { require "$d/Cargo.toml" "$g" "tools/check.sh's $g group builds in $d" || bad=1; first=0; }
          normpath "$d/$tok"
          require "$NP" "$g" "tools/check.sh's $g group runs $tok in $d" || bad=1
        fi
        case "$tok" in *.sh | *.py) scan_script "$tok" "$g" || bad=1 ;; esac
      done
    done <<< "${blocks[$g]}"
    reads[$g]=$((STEP_EDGES - reads[$g]))
  done
  for g in $ALL_GROUPS; do
    [ "${reads[$g]:-0}" -gt 0 ] || { echo "ci-scope: the steps of group '$g' read no files" >&2; bad=1; }
  done
  # Rust sources that include or read other files: include_bytes!, include_str!, include!, #[path] (indented too) and file reads
  # through string literals. Test code needs only the rust group, which runs the host tests.
  while IFS= read -r f; do
    case "$f" in */tests/* | */tests.rs) g=rust ;; *) groups_cached "$f"; g="$GC" ;; esac
    crate="$(dirname "$f")"
    while [ "$crate" != . ] && [ ! -f "$crate/Cargo.toml" ]; do crate="$(dirname "$crate")"; done
    scan_refs "$f" "$g" 1 "$(dirname "$f")" "$crate" || bad=1
  done < <(git grep -lE 'include_bytes!|include_str!|include!|#\[path|fs::read|File::open|CARGO_MANIFEST_DIR' -- '*.rs')
  # The font tests run in fonts/ and read files of the repository (docs/PALETTE.md and the like).
  while IFS= read -r f; do
    scan_bun "$f" || bad=1
  done < <(git ls-files 'fonts/*.ts')
  echo "ci-scope: group steps: $STEP_EDGES files checked" >&2
  return "$bad"
}

# changed_since REF: the paths changed on this branch since its merge base with REF (renames count as a delete and an add)
changed_since() {
  git rev-parse --verify --quiet "$1^{commit}" > /dev/null || fail "cannot resolve '$1' (fetch it, with enough history for a merge base)"
  git diff --name-only --no-renames "$1...HEAD" || fail "cannot diff against '$1'"
}

# expand ANSWER: `all` spelled out as every group, anything else as it is
expand() { if [ "$1" = all ]; then echo "${ALL_GROUPS// /,}"; else echo "$1"; fi; }

case "${1:-}" in
  --self-test) [ $# -eq 1 ] || { echo "ci-scope: --self-test takes no argument" >&2; exit 2; }; self_test ;;
  --steps) steps_closure ;;
  --files) [ $# -eq 1 ] || { echo "ci-scope: --files takes no argument (the paths come from standard input)" >&2; exit 2; }; answer="$(classify)"; expand "$answer" ;;
  --base) [ $# -eq 2 ] || { echo "ci-scope: --base needs exactly one ref" >&2; exit 2; }; paths="$(changed_since "$2")"; answer="$(classify <<< "$paths")"; expand "$answer" ;;
  "") paths="$(changed_since origin/main)"; answer="$(classify <<< "$paths")"; expand "$answer" ;;
  *) fail "unknown argument $1 (see the usage at the top of the script)" ;;
esac
