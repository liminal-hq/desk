#!/bin/bash
# The module loader's checks that need no emulator: the module workspaces' format, lints and tests, the two cross builds of the
# no_std crates, the test modules built, packed and compared with their expected description, relocation proved by linking at a second
# base, `file` naming the packer's output, and the negative runs that must fail.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-modules.sh        (in the CI image, which has Rust with rust-src, rust-lld, file, gcc and objdump)
#        DMD_REQUIRE_REAL_MODULE=1 is set here: the packer's real-linker test must run, not skip.
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
for tool in objdump strings gcc file; do
  command -v "$tool" > /dev/null || fail "$tool is missing: the checks below need it (the CI image has it)"
done

# Succeeds if the ELF file or object has an instruction a module must not use. objdump's output is captured first, so an objdump that
# fails stops the check instead of reading as a clean file, and an output with no disassembly at all is an error too.
uses_forbidden_instruction() {
  local listing
  listing=$(objdump -d "$1") || fail "objdump failed on $1"
  grep -q 'Disassembly of section' <<< "$listing" || fail "objdump printed no disassembly of $1"
  grep -Eq '\b(cmpxchg|cmpxchg8b|cli|sti)\b' <<< "$listing"
}

# Succeeds if a file names a path of the machine that built it: a home or temporary directory, the CI image's /w checkout, or the
# directories this run built from. Image paths are mapped to /desk, /rust and /cargo by tools/build-modules.sh.
names_a_build_host_path() {
  local text
  text=$(strings -a "$1") || fail "strings failed on $1"
  grep -Eq '(^|[^A-Za-z0-9_.])/(home|w|root|tmp|Users|builds?)/' <<< "$text" && return 0
  local dir
  for dir in "$@"; do
    [ "$dir" = "$1" ] && continue
    grep -qF -- "$dir" <<< "$text" && return 0
  done
  return 1
}

step "Module loader: host tests and no_std builds"
(cd module && cargo fmt --check)
(cd module && cargo clippy --locked --all-targets -- -D warnings)
(cd module && DMD_REQUIRE_REAL_MODULE=1 cargo test --locked --quiet)
# dmdpack is a host tool; the crates that run in the core and in a module are the ones built for the two 486 targets.
(cd module && RUSTC_BOOTSTRAP=1 cargo clippy --locked -p desk-module -p desk-guest --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec -- -D warnings)
(cd module && RUSTC_BOOTSTRAP=1 cargo check --locked -p desk-module -p desk-guest --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec)

step "Test modules: format, lints, build and pack"
(cd module/mods && cargo fmt --check)
(cd module/mods && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)
rm -rf dist/modules dist/modules-0080 dist/modules-3000 dist/modules-norelocs dist/modules-repro
tools/build-modules.sh
dmdpack=${CARGO_TARGET_DIR:-module/target}/release/dmdpack
modules=dist/modules
python3 -I tools/native-check.py tests/module/hello-info.txt <("$dmdpack" info $modules/HELLO.DMD) || fail "dmdpack info HELLO.DMD differs from tests/module/hello-info.txt"
"$dmdpack" info $modules/APPMAIN.DKX | grep -q '^kind          application (.DKX)$' || fail "APPMAIN.DKX is not an application"

step "Reproducible: the same bytes from two checkouts, and no build-host path in an image"
# Two copies of the sources at different paths (one longer than the other) build the same packed files: the panic handler's file name and the
# toolchain's paths are in the image, so only the path mapping of tools/build-modules.sh makes the bytes independent of the checkout.
repro="$PWD/dist/modules-repro"
rm -rf "$repro"
for name in a/desk b/a-longer-checkout-name/desk; do
  mkdir -p "$repro/$name"
  tar --exclude=target -cf - module kernel/i486-desk-none.json tools/build-modules.sh | tar -xf - -C "$repro/$name"
  (cd "$repro/$name" && CARGO_TARGET_DIR="$repro/module-target" tools/build-modules.sh --out "$repro/$name/out")
done
# (The linked ELF files are debugging aids and are not compared: rustc mangles symbol names with a hash of the crate's location, so their
# symbol tables differ between checkouts. Nothing of that is in the packed files, which carry only the image.)
for file in HELLO.DMD APPMAIN.DKX; do
  cmp "$repro/a/desk/out/$file" "$repro/b/a-longer-checkout-name/desk/out/$file" || fail "$file differs between two checkouts"
  cmp "$repro/a/desk/out/$file" "$modules/$file" || fail "$file built in $PWD differs from the same build in another checkout"
done
for file in HELLO.DMD APPMAIN.DKX hello.elf appmain.elf; do
  if names_a_build_host_path "$modules/$file" "$PWD" "$(rustc --print sysroot)" "${CARGO_HOME:-$HOME/.cargo}"; then
    fail "$file names a path of the machine that built it: $(strings -a "$modules/$file" | grep -E '/' | head -3)"
  fi
done
# The scan must be able to fail: a file naming a home directory, a /w checkout or the current directory is caught.
printf 'panicked at /home/someone/desk/module/guest/src/lib.rs:12\0' > "$repro/leaky-home.bin"
printf 'panicked at /w/src/module/guest/src/lib.rs:12\0' > "$repro/leaky-w.bin"
printf 'panicked at %s/module/guest/src/lib.rs:12\0' "$PWD" > "$repro/leaky-cwd.bin"
for leak in home w cwd; do
  names_a_build_host_path "$repro/leaky-$leak.bin" "$PWD" || fail "the path scan missed a leaked $leak path"
done
echo "identical from two checkouts; no build-host path in HELLO.DMD, APPMAIN.DKX or their ELF files; the scan catches planted paths"

step "Disassembly: no cmpxchg, no cli or sti"
for elf in hello appmain; do
  if uses_forbidden_instruction "$modules/$elf.elf"; then
    fail "$elf.elf contains an instruction a module must not use"
  fi
done
echo "no cmpxchg, cli or sti in hello.elf or appmain.elf"
# The scan must be able to fail: an object with each of them is refused, and a clean one is not.
cat > "$repro/bad.c" <<'C'
void bad(int *p) { __asm__ volatile("lock cmpxchgl %1, (%0)" : : "r"(p), "r"(1) : "eax", "memory"); __asm__ volatile("cli"); __asm__ volatile("sti"); }
C
printf 'int fine(int x) { return x + 1; }\n' > "$repro/fine.c"
gcc -m32 -O1 -c "$repro/bad.c" -o "$repro/bad.o"
gcc -m32 -O1 -c "$repro/fine.c" -o "$repro/fine.o"
uses_forbidden_instruction "$repro/bad.o" || fail "the instruction scan missed cmpxchg, cli and sti"
! uses_forbidden_instruction "$repro/fine.o" || fail "the instruction scan refused a clean object"
objdump -d "$repro/bad.o" | grep -Eo '\b(cmpxchg|cli|sti)[a-z]*' | sort -u | tr '\n' ' '
echo "<- the scan refuses an object that has them"

step "Relocation: the same module linked at a second base"
tools/build-modules.sh --link-base 0x00800000 --out dist/modules-0080
"$dmdpack" cross $modules/HELLO.DMD dist/modules-0080/HELLO.DMD
"$dmdpack" cross dist/modules-0080/HELLO.DMD $modules/HELLO.DMD
"$dmdpack" cross $modules/APPMAIN.DKX dist/modules-0080/APPMAIN.DKX
# A move by 0x3000 changes the second byte of every relocated word as well as the high ones (a move by 0x400000 leaves the low
# 16 bits alone and changes byte 2 only; 0x40400000, a move of 0x40000000 more, changes byte 3). A link base is a multiple of 4096, so the low 12 bits cannot change; the packer's tests cover each byte.
rm -rf dist/modules-3000
tools/build-modules.sh --link-base 0x00403000 --out dist/modules-3000
"$dmdpack" cross $modules/HELLO.DMD dist/modules-3000/HELLO.DMD
"$dmdpack" cross dist/modules-3000/HELLO.DMD dist/modules-0080/HELLO.DMD
"$dmdpack" cross $modules/APPMAIN.DKX dist/modules-3000/APPMAIN.DKX
rm -rf dist/modules-4040
tools/build-modules.sh --link-base 0x40400000 --out dist/modules-4040
"$dmdpack" cross $modules/HELLO.DMD dist/modules-4040/HELLO.DMD
"$dmdpack" cross dist/modules-4040/HELLO.DMD dist/modules-3000/HELLO.DMD
# Two files linked at one base prove nothing, so cross refuses them with a clear error.
if "$dmdpack" cross $modules/HELLO.DMD $modules/HELLO.DMD 2> dist/modules-3000/same.err; then
  fail "cross accepted two files linked at the same base"
fi
grep -q 'both files are linked at 0x400000' dist/modules-3000/same.err || fail "unexpected refusal: $(cat dist/modules-3000/same.err)"

# One relocated word of the second link changed (and the checksum made right): the cross check must refuse and name the offset.
python3 -I - dist/modules-0080/HELLO.DMD dist/modules-0080/TAMPER.DMD <<'PY'
import struct, sys, zlib
src, dst = sys.argv[1], sys.argv[2]
b = bytearray(open(src, "rb").read())
image, reloc = struct.unpack_from("<I", b, 24)[0], struct.unpack_from("<I", b, 52)[0]
word = image + struct.unpack_from("<I", b, reloc)[0]
b[word] ^= 0x10
b[84:88] = bytes(4)
struct.pack_into("<I", b, 84, zlib.crc32(bytes(b)))
open(dst, "wb").write(b)
PY
if "$dmdpack" cross $modules/HELLO.DMD dist/modules-0080/TAMPER.DMD 2> dist/modules-0080/cross.err; then
  fail "cross accepted a second link with a changed relocated word"
fi
grep -q 'differs from the second at image offset 0x' dist/modules-0080/cross.err || fail "cross did not name the differing offset: $(cat dist/modules-0080/cross.err)"

# A link without --emit-relocs: no relocation sections, so the packer lists none and cannot tell. The cross check is what catches it.
mkdir -p dist/modules-norelocs
# (RUSTFLAGS replaces the config file's flags; a --config array would be joined to them.)
(cd module/mods && RUSTFLAGS='-Clink-arg=-nostdlib -Clink-arg=-zmax-page-size=0x1000' RUSTC_BOOTSTRAP=1 \
  cargo build --release --locked --quiet --target-dir "$PWD/../../dist/modules-norelocs/.target")
"$dmdpack" pack dist/modules-norelocs/.target/i486-desk-none/release/hello dist/modules-norelocs/HELLO.DMD
if "$dmdpack" cross dist/modules-norelocs/HELLO.DMD dist/modules-0080/HELLO.DMD 2> dist/modules-norelocs/cross.err; then
  fail "cross accepted a module linked without --emit-relocs"
fi
echo "refused as expected: $(cat dist/modules-norelocs/cross.err)"

step "Packer refusals"
# The second link is not at the default base, and an object file is not an executable.
if "$dmdpack" pack dist/modules-0080/hello.elf dist/modules-0080/NOBASE.DMD 2> dist/modules-0080/pack.err; then
  fail "pack accepted a link at 0x00800000 without --link-base"
fi
grep -q 'linked at 0x800000, not at 0x400000' dist/modules-0080/pack.err || fail "unexpected refusal: $(cat dist/modules-0080/pack.err)"
"$dmdpack" pack --link-base 0x00800000 dist/modules-0080/hello.elf dist/modules-0080/AGAIN.DMD > /dev/null
cmp dist/modules-0080/AGAIN.DMD dist/modules-0080/HELLO.DMD || fail "packing the same ELF twice gave two files"
printf 'int answer(int x) { return x + 42; }\n' > dist/modules-0080/object.c
gcc -m32 -c dist/modules-0080/object.c -o dist/modules-0080/object.o
if "$dmdpack" pack dist/modules-0080/object.o dist/modules-0080/OBJECT.DMD 2> dist/modules-0080/object.err; then
  fail "pack accepted an object file"
fi
grep -q 'ET_EXEC' dist/modules-0080/object.err || fail "unexpected refusal: $(cat dist/modules-0080/object.err)"
[ ! -e dist/modules-0080/OBJECT.DMD ] || fail "a refused input left an output file"
echo "refused as expected: $(cat dist/modules-0080/pack.err); $(cat dist/modules-0080/object.err)"

step "file(1) names the packer's output"
cp $modules/HELLO.DMD dist/modules/DAMAGED.DMD
printf '\x00' | dd of=dist/modules/DAMAGED.DMD bs=1 count=1 conv=notrunc status=none
{
  file -b -m tools/desk.magic $modules/HELLO.DMD
  file -b -m tools/desk.magic $modules/APPMAIN.DKX
  file -b -m tools/desk.magic dist/modules/DAMAGED.DMD
} > dist/modules/file-magic.txt
diff tests/module/file-magic.txt dist/modules/file-magic.txt || fail "file -m tools/desk.magic disagrees with tests/module/file-magic.txt"
cat dist/modules/file-magic.txt

step "Negative runs: the comparisons must be able to fail"
sed 's/^alignment     16$/alignment     32/' tests/module/hello-info.txt > dist/modules/info-wrong.txt
if python3 -I tools/native-check.py dist/modules/info-wrong.txt <("$dmdpack" info $modules/HELLO.DMD) > /dev/null; then
  fail "the info comparison accepted a wrong alignment"
fi
sed '1s/module/application/' tests/module/file-magic.txt > dist/modules/file-magic-wrong.txt
if diff dist/modules/file-magic-wrong.txt dist/modules/file-magic.txt > /dev/null; then
  fail "the file-magic comparison accepted a wrong kind"
fi
echo "negative runs refused as expected"
