#!/bin/bash
# Builds the module loader's test modules and packs them: dmdpack (host), the modules for the kernel's target, then HELLO.DMD and
# APPMAIN.DKX in dist/modules with each linked ELF beside them for debugging (docs/LOADER.md, "Building a module").
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/build-modules.sh [--link-base ADDR] [--out DIR]
#
# With --link-base the modules are linked at ADDR instead of 0x00400000 and packed with `--link-base ADDR`; the build goes in its own
# target directory under DIR, so the normal build stays warm. `dmdpack cross` compares the two builds (tools/check-modules.sh).
# Needs the CI image (docs/TOOLCHAIN.md): Rust with rust-src, for -Zbuild-std.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)

out=dist/modules
base=
while [ $# -gt 0 ]; do
  case "$1" in
    --link-base) base=${2:?--link-base needs an address}; shift 2 ;;
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    *) echo "usage: tools/build-modules.sh [--link-base ADDR] [--out DIR]" >&2; exit 2 ;;
  esac
done

# name:kind pairs; the packed file is the name in upper case with .DMD for a module and .DKX for an application.
modules=(hello:module appmain:app)

target=${CARGO_TARGET_DIR:-module/target}
cargo build --release --locked --quiet --manifest-path module/Cargo.toml -p dmdpack
dmdpack="$target/release/dmdpack"

mkdir -p "$out"
mods_target=module/mods/target
pack_args=()
if [ -n "$base" ]; then
  mods_target="$out/.target"
  pack_args=(--link-base "$base")
  export DESK_LINK_BASE="$base"
fi
(cd module/mods && RUSTC_BOOTSTRAP=1 cargo build --release --locked --quiet --target-dir "$root/$mods_target")

for entry in "${modules[@]}"; do
  name=${entry%%:*}
  kind=${entry##*:}
  elf="$mods_target/i486-desk-none/release/$name"
  upper=$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')
  case "$kind" in
    module) file="$upper.DMD" ;;
    app) file="$upper.DKX" ;;
  esac
  "$dmdpack" pack --kind "$kind" ${pack_args[@]+"${pack_args[@]}"} "$elf" "$out/$file"
  cp "$elf" "$out/$name.elf"
done
