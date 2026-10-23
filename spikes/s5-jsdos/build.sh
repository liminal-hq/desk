#!/bin/bash
# Builds the s5 probes and every js-dos bundle, then stages the browser site in dist/s5/site/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/s5-jsdos/build.sh        (after tools/setup.sh and spikes/s5-jsdos/fetch-jsdos.sh)
#
#   tools/build.sh                           --> dist/DESK.EXE, dist/CWSDPMI.EXE        (C layer plus Rust core)
#   spikes/d0-rust-dos/build.sh              --> dist/d0/HELLO.EXE, HW.EXE, solo/HELLO.EXE (pure Rust)
#   spikes/s5-jsdos/probes (cargo, rust-lld) --> dist/s5/VGA13.EXE, VBE.EXE             (packed by tools/elf2go32.py)
#   spikes/s5-jsdos/bundle.py                --> dist/s5/site/bundles/*.jsdos
#   page/ + vendor/js-dos                    --> dist/s5/site/                          (what a Pages site would serve)
#
# One bundle per scenario: each runs one program from S5RUN.BAT, which records the exit status in EXITCODE.TXT and then writes
# DONE.TXT, the file a harness polls for. The program's own standard output is not redirected, so the emulator's stdout event
# sees it.
set -euo pipefail
cd "$(dirname "$0")/../.."
S5=spikes/s5-jsdos
OUT=dist/s5
SITE=$OUT/site
[ -f vendor/js-dos/js-dos.js ] || { echo "build: no js-dos in vendor/; run $S5/fetch-jsdos.sh" >&2; exit 1; }

tools/build.sh
spikes/d0-rust-dos/build.sh
spikes/s1-dxe3/build.sh

# The probes: the same target, linker script and packer as the d0 programs.
(cd $S5/probes && RUSTC_BOOTSTRAP=1 cargo build --release --locked)
mkdir -p "$OUT/obj" "$SITE/bundles"
STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null
for p in vga13 vbe tone; do
  tools/elf2go32.py "$OUT/obj/stub.exe" "$S5/probes/target/dos-elf/release/$p" "$OUT/$(echo "$p" | tr '[:lower:]' '[:upper:]').EXE"
done

# S5RUN.BAT: run the command, keep its exit status (a ladder of IF ERRORLEVEL tests, as tools/dosrun.sh does, because the emulator
# does not expand %ERRORLEVEL%), then say it is finished.
batch() { # <command line>
  {
    echo '@echo off'
    echo 'echo S5-BEGIN'
    echo "$1"
    echo 'echo S5-END'
    echo 'set RC=0'
    for n in $(seq 1 255); do echo "if errorlevel $n set RC=$n"; done
    echo 'echo %RC%> EXITCODE.TXT'
    echo 'echo done> DONE.TXT'
  } | sed 's/$/\r/' > "$OUT/obj/S5RUN.BAT"
}
# bundle <name> <command line> [bundle.py options and files...]
bundle() {
  local name=$1 cmd=$2
  shift 2
  batch "$cmd"
  $S5/bundle.py --out "$SITE/bundles/$name.jsdos" --run "call s5run.bat" "$@" "$OUT/obj/S5RUN.BAT=S5RUN.BAT"
}

CWS=dist/CWSDPMI.EXE
cp $CWS $OUT/CWSDPMI.EXE # so tools/dosrun.sh dist/s5 can run the probes natively
bundle desk 'DESK.EXE' dist/DESK.EXE $CWS
bundle hello 'HELLO.EXE' dist/d0/HELLO.EXE $CWS
bundle hello-exit7 'HELLO.EXE exit 7' dist/d0/HELLO.EXE $CWS
bundle hello-solo 'HELLO.EXE' dist/d0/solo/HELLO.EXE
bundle hw 'HW.EXE' dist/d0/HW.EXE $CWS
bundle vga13 'VGA13.EXE' $OUT/VGA13.EXE $CWS
bundle vbe 'VBE.EXE' $OUT/VBE.EXE $CWS
bundle tone 'TONE.EXE' $OUT/TONE.EXE $CWS
bundle tone-compat 'TONE.EXE' --set sblaster.oplemu=compat $OUT/TONE.EXE $CWS
bundle tone-default 'TONE.EXE' --set sblaster.oplemu=default $OUT/TONE.EXE $CWS
bundle vbe-banked 'VBE.EXE banked' $OUT/VBE.EXE $CWS
# The same probes with the settings of a native run (tools/dosrun.sh): the interpreter and a 486 that faults on newer instructions.
bundle hello-486 'HELLO.EXE' --set cpu.core=normal --set cpu.cputype=486 dist/d0/HELLO.EXE $CWS
bundle vbe-nolfb 'VBE.EXE' --set dosbox.machine=vesa_nolfb dist/s5/VBE.EXE $CWS
bundle vbe-oldvbe 'VBE.EXE' --set dosbox.machine=vesa_oldvbe dist/s5/VBE.EXE $CWS
# What the emulator says about itself: the DOS version, memory and the settings that matter for comparing with a native run.
bundle info 'ver
mem
config -get machine
config -get memsize
config -get core
config -get cputype
config -get cycles
config -get oplemu
config -get oplmode
config -get sbtype
config -get vmemsize
config -get vesa' dist/s5/VBE.EXE
# The s1 module demo: HOST.EXE loads the DXE3 modules from the current directory.
S1FILES=$(ls dist/s1/*.DXE)
bundle s1-life 'HOST.EXE life' dist/s1/HOST.EXE $CWS $S1FILES
bundle s1-panic 'HOST.EXE panic' dist/s1/HOST.EXE $CWS $S1FILES
bundle s1-fault-div-catch 'HOST.EXE fault div catch' dist/s1/HOST.EXE $CWS $S1FILES

# The site: the page and harness, js-dos with its licence, and the bundles.
rm -rf "$SITE/vendor"
mkdir -p "$SITE/vendor"
cp -r vendor/js-dos "$SITE/vendor/js-dos"
cp $S5/page/index.html $S5/page/harness.js "$SITE/"
du -sh "$SITE" "$SITE/bundles"
ls -l "$SITE/bundles"
