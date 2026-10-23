#!/bin/bash
# Everything the CI runs: licence headers, markdown style and links, the progress report generator's tests, the font build check (skipped where bun is not set up), Rust format
# and lints, the HAL's and the shell's host tests and cross builds, the frame tools' tests, the DOS build and a headless run, the s1-dxe3 module scenarios, the d0-rust-dos
# checks, the e-std-stubs checks, the s5-jsdos probe lints, the a-vga-vbe checks, the b-dos-input checks, the shell on DOS, then the native boot test under QEMU, then the comparison of the two tracks' frames.
# With NATIVE_EXTRA=1 it also runs the native boot test under Bochs as a 486, which needs Docker (not in the CI image).
# JSDOS=1 adds the browser run of the s5-jsdos spike (network, Node and Chromium needed).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check.sh        (run tools/setup.sh once first)
#        NATIVE_EXTRA=1 tools/check.sh    (adds the Bochs run)
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

step "Licence headers"
tools/check-headers.sh

step "Markdown style and links"
python3 -I tools/check-docs.py

step "Progress report generator: unit tests and a dry run of the data gather"
python3 -I tools/progress-report/tests/test_report.py
python3 -I tools/progress-report/gather.py --dry-run

step "Fonts"
if command -v bun >/dev/null && [ -d fonts/node_modules ]; then
  (cd fonts && bun build.ts --check && bun test)
else
  echo "skipped: needs bun and \`bun install\` in fonts/"
fi

step "Rust format and lints"
(cd core && cargo fmt --check)
(cd core && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

# The shared HAL and drivers: host tests against the mock port bus, then the no_std crates built for the kernel's target and for the
# pure-Rust DOS target (cargo check; clippy for the kernel target, where the x86 port instructions are compiled).
step "HAL: host tests and no_std builds"
(cd hal && cargo fmt --check)
(cd hal && cargo clippy --locked --all-targets -- -D warnings)
(cd hal && cargo test --locked --quiet)
(cd hal && RUSTC_BOOTSTRAP=1 cargo clippy --locked --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd hal && RUSTC_BOOTSTRAP=1 cargo check --locked --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)

# The shell's crates (desk-gfx and those that follow): host tests and the same two cross builds as the HAL's. See docs/FIRST-WINDOW.md.
step "Shell: host tests and no_std builds"
(cd shell && cargo fmt --check)
(cd shell && cargo clippy --locked --all-targets -- -D warnings)
(cd shell && cargo test --locked --quiet)
(cd shell && RUSTC_BOOTSTRAP=1 cargo clippy --locked --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd shell && RUSTC_BOOTSTRAP=1 cargo check --locked --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)
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

step "Frame tools: PNG, PPM and digest reading, checks and the monitor script"
python3 -I tools/test_frames.py

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

step "Spike s1-dxe3: Rust format and lints for the modules"
(cd spikes/s1-dxe3/modules && cargo fmt --check)
(cd spikes/s1-dxe3/modules && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Spike s1-dxe3: build the Rust modules (DXE3) and the host"
spikes/s1-dxe3/build.sh

step "Spike s1-dxe3: load, call, crash and unload modules in headless DOSBox"
spikes/s1-dxe3/run.sh

step "Spike d0-rust-dos: pure Rust DOS programs, packed with no C layer"
spikes/d0-rust-dos/check.sh

step "Spike s5-jsdos: Rust format and lints for the probes"
(cd spikes/s5-jsdos/probes && cargo fmt --check)
(cd spikes/s5-jsdos/probes && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Spike e-std-stubs: std on an invented OS, stubs and a patched platform layer"
spikes/e-std-stubs/check.sh

step "Spike a-vga-vbe: VGA modes 13h and 12h and VBE 2.0 through desk-vga, on the pure Rust DOS runtime"
spikes/a-vga-vbe/check.sh

step "Spike b-dos-input: the keyboard on IRQ 1 and the mouse on INT 33h on the pure Rust DOS runtime, keys typed in by AUTOTYPE"
spikes/b-dos-input/check.sh

# The mouse needs a pointer. DOSBox has no scripted injection, so this drives the SDL window under a virtual X server with xdotool, in a
# container built from tools/docker/Dockerfile.dos-xvfb. Off by default: the CI image has no X server or Docker, and the run takes about half a minute.
if [ -n "${INPUT_EXTRA:-}" ]; then
  step "Spike b-dos-input (opt-in, INPUT_EXTRA=1): the mouse moved and clicked by xdotool under Xvfb"
  spikes/b-dos-input/check-xvfb.sh
fi

step "Shell on DOS: the first window's scenario in DOSBox at both depths, read back and compared with the goldens"
shell/dos/check.sh

step "Native boot: NASM loader, Rust kernel, QEMU as a 486"
tools/native-test.sh

# Cheap: it only reads the dumps the two steps above kept in dist/. `tools/shell-compare.sh --run` runs both tracks first.
step "Both tracks: the DOS read-back and the QEMU screendumps are the same pictures"
tools/shell-compare.sh

if [ "${NATIVE_EXTRA:-}" = 1 ]; then
  step "Native boot: the same checks in Bochs as a 486 (opt-in, needs Docker)"
  tools/native-test-bochs.sh
fi

# Opt-in: needs the network, Node and a Chromium, none of which the CI image carries. See docs/S5-JSDOS.md.
if [ "${JSDOS:-}" = 1 ]; then
  step "Spike s5-jsdos: the DOS programs in js-dos, in headless Chromium"
  tools/check-jsdos.sh
fi

printf '\nAll checks passed.\n'
