#!/bin/bash
# The DOS runtime's checks: format and lints, the build, then every example program run in headless DOSBox against its transcript.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: dos/check.sh        (after tools/setup.sh)
#
# Transcripts live in dos/expected/; dos/tools/transcript.sh says what is normalised before comparing. The d0-rust-dos spike's own check
# (spikes/d0-rust-dos/check.sh, stage 1 linked by DJGPP's ld) builds on this one.
set -euo pipefail
cd "$(dirname "$0")/.."
DIR=dos
EXP=$DIR/expected
# shellcheck source=dos/tools/transcript.sh
. dos/tools/transcript.sh

step "Rust format and lints (dos workspace)"
(cd $DIR && cargo fmt --check)
(cd $DIR && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build: rust-lld, then dos/tools/elf2go32.py (no gcc, no ld, no C library)"
$DIR/build.sh

step "The strong memcpy and memset linked (string instructions, not compiler_builtins' thunks)"
OBJDUMP="$(command -v llvm-objdump || command -v objdump || true)"
[ -n "$OBJDUMP" ] || fail "no objdump on this machine, so the symbols in the linked image cannot be checked"
disasm="$("$OBJDUMP" -d --no-show-raw-insn dos/target/dos-elf/release/hello)"
# The body of one symbol: from its label to the next blank line.
body() { printf '%s\n' "$disasm" | awk -v sym="<$1>:" '$2 == sym {on = 1; next} on && NF == 0 {exit} on'; }
# llvm-objdump spells the dword string moves movsd and stosd; GNU objdump (the CI image has only that one) spells them movsl and `stos %eax`; the register operand is what tells the dword store from a byte one.
body memcpy | grep -Eq 'rep movs[dl]' || fail "memcpy in the linked hello has no rep movsd: the weak compiler_builtins one won"
body memset | grep -Eq 'rep stos.*eax' || fail "memset in the linked hello has no rep stosd: the weak compiler_builtins one won"
echo "memcpy contains rep movsd and memset contains rep stosd"

step "HELLO.EXE: output, arguments, heap, x87, DPMI, files"
run dist/d0 HELLO.EXE
[ "$status" = 0 ] || fail "HELLO.EXE exited with status $status"
same HELLO.EXE $EXP/hello.txt
echo "$out" | head -3

step "Exit status and arguments"
run dist/d0 HELLO.EXE exit 7
[ "$status" = 7 ] || fail "HELLO.EXE exit 7 exited with status $status"
case "$out" in *'args: 2 ["exit", "7"]'*) ;; *) fail "arguments were not passed: $out" ;; esac
echo "status $status"

step "CTX.EXE: what the stub hands over at entry"
run dist/d0 CTX.EXE one two
[ "$status" = 0 ] || fail "CTX.EXE exited with status $status"
same CTX.EXE $EXP/ctx.txt

step "HW.EXE: IRQ 0 hook and VGA mode 13h"
run dist/d0 HW.EXE
[ "$status" = 0 ] || fail "HW.EXE exited with status $status"
same HW.EXE $EXP/hw.txt
echo "$out" | sed -n 2,9p

step "A panic restores the hooked vector and exits with status 3"
run dist/d0 PANICKY.EXE
[ "$status" = 3 ] || fail "PANICKY.EXE exited with status $status, not 3"
same PANICKY.EXE $EXP/panicky.txt
echo "$out | status $status" | tail -2

step "Exit with the timer hooked and mode 13h set: the clean-up list runs on exit"
run dist/d0 HW.EXE leave-hooked
[ "$status" = 0 ] || fail "HW.EXE leave-hooked exited with status $status"
case "$out" in *'exiting with the timer hooked and mode 13h set'*) ;; *) fail "unexpected output: $out" ;; esac
case "$out" in *'the timer vector and text mode are back: true') ;; *) fail "exit did not put the machine back: $out" ;; esac
echo "$out" | tail -2

step "HW.EXE fast: a reprogrammed PIT at 60 and 560 Hz, the BIOS clock kept by chaining, the stopwatch and delay_us"
run dist/d0 HW.EXE fast
[ "$status" = 0 ] || fail "HW.EXE fast exited with status $status"
same "HW.EXE fast" $EXP/hw-fast.txt
echo "$out" | grep -E 'BIOS counter|fast ticks'

step "HW.EXE fast-full: a full clean-up list refuses hook_rate and the PIT stays at the BIOS rate"
run dist/d0 HW.EXE fast-full
[ "$status" = 0 ] || fail "HW.EXE fast-full exited with status $status"
same "HW.EXE fast-full" $EXP/hw-fast-full.txt
echo "$out" | tail -1

# The second command runs in the same DOSBox, so on the same emulated PIT: it is what shows the rate was restored.
step "Exit with the PIT at 60 Hz: the clean-up list puts the vector and the BIOS rate back"
DOS_THEN="HW.EXE rate-check" run dist/d0 HW.EXE fast-leave-hooked
[ "$status" = 0 ] || fail "HW.EXE fast-leave-hooked exited with status $status"
same "HW.EXE fast-leave-hooked" $EXP/hw-fast-leave-hooked.txt
echo "$out" | tail -2

step "A panic with the PIT at 60 Hz: the clean-up list puts the vector and the BIOS rate back"
DOS_THEN="HW.EXE rate-check" run dist/d0 HW.EXE fast-panic
[ "$status" = 3 ] || fail "HW.EXE fast-panic exited with status $status, not 3"
same "HW.EXE fast-panic" $EXP/hw-fast-panic.txt
echo "$out" | tail -2

step "CLEANUP.EXE: the list runs last in, first out, and refuses a 17th entry"
run dist/d0 CLEANUP.EXE
[ "$status" = 0 ] || fail "CLEANUP.EXE exited with status $status"
same CLEANUP.EXE $EXP/cleanup.txt
echo "$out" | tail -4

step "Allocation failure is a panic"
run dist/d0 HELLO.EXE oom
[ "$status" = 3 ] || fail "HELLO.EXE oom exited with status $status, not 3"
case "$out" in *'memory allocation of 943718400 bytes failed') ;; *) fail "unexpected output: $out" ;; esac
echo "status $status"

step "Self-contained executable (CWSDSTUB.EXE, host included) runs with no CWSDPMI.EXE beside it"
[ ! -e dist/d0/solo/CWSDPMI.EXE ] || fail "dist/d0/solo must not hold CWSDPMI.EXE"
run dist/d0/solo HELLO.EXE
[ "$status" = 0 ] || fail "solo HELLO.EXE exited with status $status"
same "solo HELLO.EXE" $EXP/hello.txt
echo "identical to HELLO.EXE"

step "Without CWSDPMI.EXE the stub says so and fails"
mkdir -p dist/d0/nohost
cp dist/d0/HELLO.EXE dist/d0/nohost/HELLO.EXE
rm -f dist/d0/nohost/CWSDPMI.EXE
run dist/d0/nohost HELLO.EXE
[ "$status" != 0 ] || fail "HELLO.EXE ran without a DPMI host"
case "$out" in *'no DPMI'*) ;; *) fail "the stub gave no message about the DPMI host: $out" ;; esac
echo "$out (status $status)"

step "A program that returns from _start does not exit cleanly"
run dist/d0 RETSTART.EXE
[ "$status" != 0 ] || fail "RETSTART.EXE exited with status 0"
echo "status $status"

step "A wrong expected transcript fails the check"
wrong="$(mktemp "${TMPDIR:-/tmp}/d0-wrong.XXXXXX")"
trap 'rm -f "$wrong"' EXIT
sed 's/squares=10/squares=11/' $EXP/hello.txt > "$wrong"
run dist/d0 HELLO.EXE
if (same HELLO.EXE "$wrong") > /dev/null 2>&1; then fail "a wrong transcript was accepted"; fi
echo "rejected, as it should be"
sed 's/6272 cases, 0 wrong/6272 cases, 1 wrong/' $EXP/hello.txt > "$wrong"
if (same HELLO.EXE "$wrong") > /dev/null 2>&1; then fail "a wrong memcpy and memset line was accepted"; fi
echo "a wrong memcpy and memset line rejected, as it should be"

step "A transcript with the clean-up order swapped fails the check"
sed -e 's/run c$/run TMP/' -e 's/run b$/run c/' -e 's/run TMP$/run b/' $EXP/cleanup.txt > "$wrong"
run dist/d0 CLEANUP.EXE
if (same CLEANUP.EXE "$wrong") > /dev/null 2>&1; then fail "a transcript with the order swapped was accepted"; fi
echo "rejected, as it should be"

printf '\nAll dos checks passed.\n'
