#!/bin/bash
# Prepares a machine to build and run Desk for DOS: the Rust toolchain pieces, then DJGPP, DOSBox Staging and CWSDPMI in vendor/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/setup.sh
#
# Safe to run again. It installs nothing system-wide: Rust comes through rustup (as pinned in rust-toolchain.toml) and the DOS tools
# go under vendor/, which is not committed. A missing host tool is reported with the package that provides it, never installed.
#
# DOS_TOOLCHAIN names a directory that already holds the tools, laid out as djgpp/, dosbox/ and cwsdpmi/. The CI image sets it to
# /opt/dos-toolchain; vendor/ then links to those directories and nothing is downloaded. Without it, tools/fetch-tools.sh
# downloads them.
set -euo pipefail
cd "$(dirname "$0")/.."

# Host tools the scripts rely on, and where to get them on Debian and Ubuntu.
missing=0
need() { # <command> <package hint>
  command -v "$1" > /dev/null || { echo "setup: missing '$1' ($2)" >&2; missing=1; }
}
need rustup "https://rustup.rs"
need python3 "python3"
need curl "curl"
need sha256sum "coreutils"
need timeout "coreutils"
need tar "tar"
need bzip2 "bzip2, to unpack DJGPP"
need xz "xz-utils, to unpack DOSBox Staging"
[ "$missing" = 0 ] || exit 1

# Rust: the pinned toolchain and its components (rust-src is what -Zbuild-std compiles core and alloc from). Recent rustup installs
# whatever rust-toolchain.toml lists; the checks after it confirm the result either way.
channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
rustup toolchain install --profile minimal --no-self-update > /dev/null 2>&1 || true
for component in rust-src rustfmt clippy; do
  rustup component add "$component" > /dev/null 2>&1 || true
done
[ "$(rustc --version | cut -d' ' -f2)" = "$channel" ] || { echo "setup: rustc is not $channel" >&2; exit 1; }
rustup component list --installed | grep -q '^rust-src' || { echo "setup: the rust-src component is missing" >&2; exit 1; }
echo "rust: $(rustc --version)"

# DOS tools.
if [ -n "${DOS_TOOLCHAIN:-}" ]; then
  mkdir -p vendor
  for tool in djgpp dosbox cwsdpmi; do
    [ -d "$DOS_TOOLCHAIN/$tool" ] || { echo "setup: $DOS_TOOLCHAIN/$tool not found" >&2; exit 1; }
    ln -sfn "$DOS_TOOLCHAIN/$tool" "vendor/$tool"
  done
  echo "vendor/: linked to $DOS_TOOLCHAIN"
else
  tools/fetch-tools.sh
fi

# DJGPP's binaries are 64-bit Linux programs that load libfl.so.2 (libfl2 on Debian and Ubuntu, flex on Arch and Fedora), so run
# one to find out now rather than in the middle of a build.
gcc="vendor/djgpp/djgpp/bin/i586-pc-msdosdjgpp-gcc"
"$gcc" --version > /dev/null 2>&1 || { echo "setup: $gcc does not run; is libfl.so.2 installed?" >&2; exit 1; }
echo "djgpp: $("$gcc" --version | head -n 1)"
dosbox=(vendor/dosbox/*/dosbox)
echo "dosbox: $("${dosbox[0]}" --version 2> /dev/null | head -n 1)"
echo "cwsdpmi: $(ls vendor/cwsdpmi/bin/CWSDPMI.EXE)"
