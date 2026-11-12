#!/bin/bash
# Everything the CI runs, in groups the CI runs as separate jobs: base (licence headers, markdown style and links, the CI scope table),
# report (the progress report generator's tests, the frame tools' tests and the goldens' own checks), fonts (the font build check),
# rust (format, lints, host tests and cross builds of core/, hal/, shell/, module/ and tools/steps-junit/, and the spike workspaces
# checked on their own), dos (the DOS build and headless runs, the s1-dxe3 modules, the d0-rust-dos, a-vga-vbe and b-dos-input spikes),
# stdstubs (the e-std-stubs spike), shelldos (the shell on DOS), native (the native boot test under QEMU) and compare (the two tracks'
# frames compared, from what shelldos and native left in dist/).
# With NATIVE_EXTRA=1 it also runs the native boot test under Bochs as a 486, which needs Docker (not in the CI image).
# INPUT_EXTRA=1 adds the mouse runs under Xvfb in Docker (spike B's and the shell's). JSDOS=1 adds the browser run of the s5-jsdos spike
# (network, Node and Chromium needed).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check.sh                      every group, in order (run tools/setup.sh once first)
#        tools/check.sh --groups LIST        only the named groups or parts, a comma list such as `rust:hal,dos`, in script order;
#                                            `all` is every group; compare needs what shelldos and native left in dist/
#        tools/check.sh --list               the groups, their parts and what each checks
#        tools/check.sh --names              the group names, one per line
#        tools/check.sh --units              every group or group:part a CI job can be given, one per line
#        NATIVE_EXTRA=1 tools/check.sh       (adds the Bochs run)
#
# CHECK_TIMES=FILE appends one line per step (`start`, milliseconds, group, title) and one at the end (`end`, milliseconds, exit
# status), tab-separated, for the CI's step report (tools/steps-junit). A relative FILE resolves against the repository root, since the
# script changes into it.
set -euo pipefail
cd "$(dirname "$0")/.."

# The groups, in order, and the parts of the groups that have them. A group without parts is one unit.
ALL_GROUPS="base report fonts rust dos stdstubs shelldos native compare"
# shellcheck disable=SC2034 # read through parts()
PARTS_rust="core hal shell module tools spikes"
# shellcheck disable=SC2034 # read through parts()
PARTS_dos="skeleton s1 d0 vga input"

parts() { local v="PARTS_$1"; printf '%s' "${!v:-}"; }

list_groups() {
  cat << 'LIST'
base       licence headers, markdown style and links, the CI scope table
report     the progress report generator's tests and dry run, the frame tools' tests, the goldens and the live scenarios' translations
fonts      the font build check (bun; fails instead of skipping when CI is set)
rust       core       core/ format and lints
           hal        hal/ format, lints, host tests and the cross builds for the kernel and DOS targets
           shell      shell/ format, lints, host tests and the cross builds
           module     module/ (the loader's format reader and packer) format, lints, host tests, the real-linker test and the format crate's cross builds
           tools      tools/steps-junit/ (the step report converter the CI posts) format, lints and host tests
           spikes     format and lints of the s1-dxe3 modules and the s5-jsdos probes
dos        skeleton   DESK.EXE built, run in headless DOSBox, and its panic path
           s1         spike s1-dxe3: the modules built, loaded, called, crashed and unloaded
           d0         spike d0-rust-dos
           vga        spike a-vga-vbe
           input      spike b-dos-input (INPUT_EXTRA=1: the Xvfb mouse run)
stdstubs   spike e-std-stubs
shelldos   the shell on DOS (INPUT_EXTRA=1: the Xvfb live run)
native     the native boot test under QEMU (NATIVE_EXTRA=1: under Bochs too, at the end)
compare    the DOS read-back against the QEMU screendumps, from dist/
LIST
}

SELECTED=all
GROUPS_GIVEN=
while [ $# -gt 0 ]; do
  case "$1" in
    --list) list_groups; exit 0 ;;
    --names) tr ' ' '\n' <<< "$ALL_GROUPS"; exit 0 ;;
    --units)
      for g in $ALL_GROUPS; do
        if [ -n "$(parts "$g")" ]; then for p in $(parts "$g"); do echo "$g:$p"; done; else echo "$g"; fi
      done
      exit 0 ;;
    --groups)
      [ $# -ge 2 ] || { echo "check: --groups needs a list (try --list)" >&2; exit 2; }
      [ -z "$GROUPS_GIVEN" ] || { echo "check: --groups given twice (join the lists with commas)" >&2; exit 2; }
      GROUPS_GIVEN=1; SELECTED="$2"; shift 2 ;;
    --groups=*)
      [ -z "$GROUPS_GIVEN" ] || { echo "check: --groups given twice (join the lists with commas)" >&2; exit 2; }
      GROUPS_GIVEN=1; SELECTED="${1#--groups=}"; shift ;;
    -h | --help) sed -n '/^# usage/,/^set -e/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "check: unknown argument '$1' (try --list)" >&2; exit 2 ;;
  esac
done
if [ "$SELECTED" != all ]; then
  [ -n "$SELECTED" ] || { echo "check: --groups needs a list (try --list)" >&2; exit 2; }
  case ",$SELECTED," in
    *,,*) echo "check: --groups has an empty item in '$SELECTED' (try --list)" >&2; exit 2 ;;
  esac
  IFS=, read -r -a picked <<< "$SELECTED"
  seen_units=","
  for u in "${picked[@]}"; do
    case "$seen_units" in *",$u,"*) echo "check: --groups names '$u' twice" >&2; exit 2 ;; esac
    seen_units="$seen_units$u,"
    g="${u%%:*}"
    case " all $ALL_GROUPS " in *" $g "*) ;; *) echo "check: unknown group '$g' (try --list)" >&2; exit 2 ;; esac
    if [ "$u" != "$g" ]; then
      case " $(parts "$g") " in *" ${u#*:} "*) ;; *) echo "check: '$g' has no part '${u#*:}' (try --list)" >&2; exit 2 ;; esac
    fi
  done
  case ",$SELECTED," in *,all,*) SELECTED=all ;; esac
fi
echo "check: groups: $SELECTED"

# whole GROUP: the whole group was asked for (by name, or by `all`)
whole() { [ "$SELECTED" = all ] || case ",$SELECTED," in *",$1,"*) true ;; *) false ;; esac; }
# run GROUP [PART]: the group, or that part of it, was asked for; the steps that follow are reported under GROUP
run() {
  whole "$1" || { [ -n "${2:-}" ] && case ",$SELECTED," in *",$1:$2,"*) true ;; *) false ;; esac; } || return 1
  CUR="$1"
}
CUR=check
step() {
  printf '\n== %s: %s\n' "$CUR" "$1"
  [ -z "${CHECK_TIMES:-}" ] || printf 'start\t%s\t%s\t%s\n' "$(date +%s%3N)" "$CUR" "$1" >> "$CHECK_TIMES"
}
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

# host_tests: the host tests of the workspace in the current directory. cargo-nextest runs them where it is installed (the CI image
# has it); with CI set it must be, because the `ci` profile's JUnit report (<workspace>/target/nextest/ci/junit.xml) goes to the pull
# request. Doc-tests, which nextest does not run, follow. Without nextest (a laptop) cargo test runs everything, as before.
ROOT=$PWD
host_tests() {
  if cargo nextest --version > /dev/null 2>&1; then
    cargo nextest run --locked --config-file "$ROOT/.config/nextest.toml" --profile "$([ -n "${CI:-}" ] && echo ci || echo default)"
    cargo test --doc --locked --quiet
  else
    [ -z "${CI:-}" ] || fail "cargo-nextest is missing (CI is set, so the host tests do not fall back to cargo test)"
    cargo test --locked --quiet
  fi
}
[ -z "${CHECK_TIMES:-}" ] || trap 'status=$?; printf "end\t%s\t%s\n" "$(date +%s%3N)" "$status" >> "$CHECK_TIMES"' EXIT

# Each group's steps are in `if run GROUP [PART]; then` blocks whose bodies are not indented: a step at the start of a line runs whenever
# its block does, an indented one is opt-in, and the manual's step table (tools/progress-report/manual/M21-evidence.md) is checked
# against that, in order. A group may have several blocks; `--groups` runs the asked-for blocks in this order.

if run base; then
step "Licence headers"
tools/check-headers.sh

step "Markdown style and links"
python3 -I tools/check-docs.py

step "CI scope table: one test per rule, the path dependencies and every tracked file covered"
tools/ci-scope.sh --self-test
fi

if run report; then
step "Progress report generator: unit tests and a dry run of the data gather"
python3 -I tools/progress-report/tests/test_report.py
python3 -I tools/progress-report/gather.py --dry-run
fi

if run fonts; then
step "Fonts"
if command -v bun > /dev/null && [ -d fonts/node_modules ]; then
  (cd fonts && bun build.ts --check && bun test)
else
  [ -z "${CI:-}" ] || fail "the font check needs bun and \`bun install\` in fonts/ (CI is set, so it does not skip)"
  echo "skipped: needs bun and \`bun install\` in fonts/"
fi
fi

if run rust core; then
step "Rust format and lints"
(cd core && cargo fmt --check)
(cd core && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)
fi

# The shared HAL and drivers: host tests against the mock port bus, then the no_std crates built for the kernel's target and for the
# pure-Rust DOS target (cargo check; clippy for the kernel target, where the x86 port instructions are compiled).
if run rust hal; then
step "HAL: host tests and no_std builds"
(cd hal && cargo fmt --check)
(cd hal && cargo clippy --locked --all-targets -- -D warnings)
(cd hal && host_tests)
(cd hal && RUSTC_BOOTSTRAP=1 cargo clippy --locked --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd hal && RUSTC_BOOTSTRAP=1 cargo check --locked --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)
fi

# The shell's crates (desk-gfx and those that follow): host tests and the same two cross builds as the HAL's. See docs/FIRST-WINDOW.md.
if run rust shell; then
step "Shell: host tests and no_std builds"
(cd shell && cargo fmt --check)
(cd shell && cargo clippy --locked --all-targets -- -D warnings)
(cd shell && host_tests)
(cd shell && RUSTC_BOOTSTRAP=1 cargo clippy --locked --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd shell && RUSTC_BOOTSTRAP=1 cargo check --locked --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)
fi

# The module loader's crates: host tests for both, and the format crate's no_std builds (the packer uses std, so the cross builds name
# -p desk-module). DMD_REQUIRE_REAL_MODULE=1 makes the packer's real-linker test (rust-src and rust-lld) fail instead of skip. See docs/LOADER.md.
if run rust module; then
step "Module loader: host tests and no_std builds"
(cd module && cargo fmt --check)
(cd module && cargo clippy --locked --all-targets -- -D warnings)
(cd module && DMD_REQUIRE_REAL_MODULE=1 host_tests)
(cd module && RUSTC_BOOTSTRAP=1 cargo clippy --locked -p desk-module --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd module && RUSTC_BOOTSTRAP=1 cargo check --locked -p desk-module --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)
fi

# The converter that turns this script's step times and log into the JUnit report on the pull request (tools/steps-junit). Every CI job
# builds and runs it after its checks, so its own tests run in this part.
if run rust tools; then
step "Step report converter: format, lints and host tests"
(cd tools/steps-junit && cargo fmt --check)
(cd tools/steps-junit && cargo clippy --locked --all-targets -- -D warnings)
(cd tools/steps-junit && host_tests)
fi

if run report; then
step "Frame tools: PNG, PPM and digest reading, checks and the monitor script"
python3 -I tools/test_frames.py
python3 -I tools/frames.py goldens tests/shell/first-window.sums tests/shell/golden/first-window
python3 -I tools/frames.py goldens tests/shell/keys.sums tests/shell/golden/keys
python3 -I tools/frames.py goldens tests/shell/live.sums tests/shell/golden/live
python3 -I tools/frames.py goldens tests/shell/live-keys.sums tests/shell/golden/live-keys
# The live scenarios translate for every emulator that can play them, so a step an emulator cannot express fails here, before any
# emulator runs. live.scn uses the mouse, which AUTOTYPE cannot send: that refusal is itself checked.
for form in --qemu --xdotool; do
  for scn in live live-keys; do
    python3 -I tools/frames.py live "tests/shell/$scn.scn" "$form" > /dev/null || fail "tests/shell/$scn.scn does not translate with $form"
  done
done
python3 -I tools/frames.py live tests/shell/live-keys.scn --autotype > /dev/null || fail "tests/shell/live-keys.scn does not translate for AUTOTYPE"
if python3 -I tools/frames.py live tests/shell/live.scn --autotype > /dev/null 2>&1; then
  fail "frames.py live accepted a mouse scenario for AUTOTYPE"
fi
fi

if run dos skeleton; then
step "Build DESK.EXE"
tools/build.sh

# The line the Rust core must produce, byte for byte. The numbers are 1^2 + ... + 10^2, that sum times 1000003, and the x87's
# square root of the sum.
EXPECTED='DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417'

step "Run DESK.EXE in headless DOSBox"
status=0
actual="$(tools/dosrun.sh dist DESK.EXE)" || status=$?
file="$(tr -d '\r' < dist/OUT.TXT 2> /dev/null || true)"
[ "$status" = 0 ] || fail "DESK.EXE exited with status $status; output: $actual"
[ "$actual" = "$EXPECTED" ] || fail "standard output is not the expected line
  expected: $EXPECTED
  actual:   $actual"
[ "$file" = "$EXPECTED" ] || fail "OUT.TXT is not the expected line
  expected: $EXPECTED
  actual:   $file"
echo "$actual"

step "A Rust panic reports through C and exits non-zero"
status=0
actual="$(tools/dosrun.sh dist DESK.EXE panic)" || status=$?
[ "$status" = 3 ] || fail "DESK.EXE panic exited with status $status, not 3; output: $actual"
case "$actual" in
  "DESK-SKELETON PANIC at line "*) ;;
  *) fail "no panic report in the output: $actual" ;;
esac
grep -q 'DESK-SKELETON PANIC' dist/OUT.TXT || fail "the panic report is missing from OUT.TXT"
echo "$actual (exit status $status)"
fi

if run rust spikes; then
step "Spike s1-dxe3: Rust format and lints for the modules"
(cd spikes/s1-dxe3/modules && cargo fmt --check)
(cd spikes/s1-dxe3/modules && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)
fi

if run dos s1; then
step "Spike s1-dxe3: build the Rust modules (DXE3) and the host"
spikes/s1-dxe3/build.sh

step "Spike s1-dxe3: load, call, crash and unload modules in headless DOSBox"
spikes/s1-dxe3/run.sh
fi

if run dos d0; then
step "Spike d0-rust-dos: pure Rust DOS programs, packed with no C layer"
spikes/d0-rust-dos/check.sh
fi

if run rust spikes; then
step "Spike s5-jsdos: Rust format and lints for the probes"
(cd spikes/s5-jsdos/probes && cargo fmt --check)
(cd spikes/s5-jsdos/probes && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)
fi

if run stdstubs; then
step "Spike e-std-stubs: std on an invented OS, stubs and a patched platform layer"
spikes/e-std-stubs/check.sh
fi

if run dos vga; then
step "Spike a-vga-vbe: VGA modes 13h and 12h and VBE 2.0 through desk-vga, on the pure Rust DOS runtime"
spikes/a-vga-vbe/check.sh
fi

if run dos input; then
step "Spike b-dos-input: the keyboard on IRQ 1 and the mouse on INT 33h on the pure Rust DOS runtime, keys typed in by AUTOTYPE"
spikes/b-dos-input/check.sh

# The mouse needs a pointer. DOSBox has no scripted injection, so this drives the SDL window under a virtual X server with xdotool, in a
# container built from tools/docker/Dockerfile.dos-xvfb. Off by default: the CI image has no X server or Docker, and the run takes about half a minute.
if [ -n "${INPUT_EXTRA:-}" ]; then
  step "Spike b-dos-input (opt-in, INPUT_EXTRA=1): the mouse moved and clicked by xdotool under Xvfb"
  spikes/b-dos-input/check-xvfb.sh
fi
fi

if run shelldos; then
step "Shell on DOS: the first window's scenario in DOSBox at both depths, read back and compared with the goldens"
shell/dos/check.sh

# Opt-in like the spike B mouse run: the live scenario's pointer, drag, Alt+Tab, typing and Alt+F4 through DOSBox's window under Xvfb in
# Docker, held to the goldens, with quick clicks and a negative run. Without Docker it says it was skipped.
if [ -n "${INPUT_EXTRA:-}" ]; then
  step "Shell on DOS (opt-in, INPUT_EXTRA=1): the live scenario's mouse and keys through DOSBox's window under Xvfb"
  shell/dos/check-xvfb.sh
fi
fi

if run native; then
step "Native boot: NASM loader, Rust kernel, QEMU as a 486"
tools/native-test.sh
fi

# Cheap: it only reads the dumps the two groups above kept in dist/. `tools/shell-compare.sh --run` runs both tracks first.
if run compare; then
step "Both tracks: the DOS read-back and the QEMU screendumps are the same pictures"
tools/shell-compare.sh
fi

# The opt-in reruns of a whole group come last, so the Bochs run never rewrites dist/shell-native-* before the comparison. Each runs
# only when its whole group was asked for.
if [ "${NATIVE_EXTRA:-}" = 1 ] && whole native; then
  CUR=native
  step "Native boot: the same checks in Bochs as a 486 (opt-in, needs Docker)"
  tools/native-test-bochs.sh
fi

# Opt-in: needs the network, Node and a Chromium, none of which the CI image carries. See docs/S5-JSDOS.md.
if [ "${JSDOS:-}" = 1 ] && whole dos; then
  CUR=dos
  step "Spike s5-jsdos: the DOS programs in js-dos, in headless Chromium"
  tools/check-jsdos.sh
fi

if [ "$SELECTED" = all ]; then printf '\nAll checks passed.\n'; else printf '\nChecks passed: %s.\n' "$SELECTED"; fi
