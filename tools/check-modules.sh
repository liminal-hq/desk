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
rm -rf dist/modules dist/modules-0080 dist/modules-norelocs
tools/build-modules.sh
dmdpack=${CARGO_TARGET_DIR:-module/target}/release/dmdpack
modules=dist/modules
python3 -I tools/native-check.py tests/module/hello-info.txt <("$dmdpack" info $modules/HELLO.DMD) || fail "dmdpack info HELLO.DMD differs from tests/module/hello-info.txt"
"$dmdpack" info $modules/APPMAIN.DKX | grep -q '^kind          application (.DKX)$' || fail "APPMAIN.DKX is not an application"

step "Disassembly: no cmpxchg, no cli or sti"
for elf in hello appmain; do
  if objdump -d "$modules/$elf.elf" | grep -E '\b(cmpxchg|cmpxchg8b|cli|sti)\b'; then
    fail "$elf.elf contains an instruction a module must not use"
  fi
done
echo "no cmpxchg, cli or sti in hello.elf or appmain.elf"

step "Relocation: the same module linked at a second base"
tools/build-modules.sh --link-base 0x00800000 --out dist/modules-0080
"$dmdpack" cross $modules/HELLO.DMD dist/modules-0080/HELLO.DMD
"$dmdpack" cross dist/modules-0080/HELLO.DMD $modules/HELLO.DMD
"$dmdpack" cross $modules/APPMAIN.DKX dist/modules-0080/APPMAIN.DKX

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
