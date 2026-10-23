#!/bin/bash
# Downloads the DOS toolchain into vendor/ for a developer's machine: the DJGPP cross-compiler, DOSBox Staging and CWSDPMI.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/fetch-tools.sh
#
# The URLs and SHA-256 sums below are copied from the `ci-dos` stage of docker/ci/Dockerfile in liminal-hq/.github, the image the
# CI builds in, so a local build and a CI build use byte-identical tools. When that Dockerfile changes a version, change it here
# in the same pull request. The result has the same layout as the image's /opt/dos-toolchain: vendor/djgpp, vendor/dosbox and
# vendor/cwsdpmi. Downloads are cached in vendor/dl and verified every time, so a corrupt or tampered file is never unpacked.
set -euo pipefail
cd "$(dirname "$0")/.."

DJGPP_URL=https://github.com/andrewwutw/build-djgpp/releases/download/v3.4/djgpp-linux64-gcc1220.tar.bz2
DJGPP_SHA256=8464f17017d6ab1b2bb2df4ed82357b5bf692e6e2b7fee37e315638f3d505f00
DOSBOX_URL=https://github.com/dosbox-staging/dosbox-staging/releases/download/v0.83.0/dosbox-staging-linux-x86_64-v0.83.0.tar.xz
DOSBOX_SHA256=d3a94f7f1c3e68a47ec88d61145506c7904452adb0c9c5928cb8cfe2331d6c5c
CWSDPMI_URL=https://www.delorie.com/pub/djgpp/current/v2misc/csdpmi7b.zip
CWSDPMI_SHA256=deacda0488e1cdd7c4a9f32fab45662b34c0ed6b2d7d4d13bc07041b62004a8c

if [ -x vendor/djgpp/djgpp/bin/i586-pc-msdosdjgpp-gcc ] && [ -n "$(ls vendor/dosbox/*/dosbox 2>/dev/null)" ] \
  && [ -f vendor/cwsdpmi/bin/CWSDPMI.EXE ]; then
  echo "vendor/: toolchain already present"
  exit 0
fi

mkdir -p vendor/dl vendor/djgpp vendor/dosbox vendor/cwsdpmi
cd vendor/dl

fetch() { # <url> <sha256>: download into the cache if missing, then verify
  local file="${1##*/}"
  [ -f "$file" ] || curl -fsSL -o "$file" "$1"
  printf '%s  %s\n' "$2" "$file" | sha256sum -c -
}
fetch "$DJGPP_URL" "$DJGPP_SHA256"
fetch "$DOSBOX_URL" "$DOSBOX_SHA256"
fetch "$CWSDPMI_URL" "$CWSDPMI_SHA256"

tar -xjf "${DJGPP_URL##*/}" -C ../djgpp
tar -xJf "${DOSBOX_URL##*/}" -C ../dosbox
python3 -I -c "import sys,zipfile;zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "${CWSDPMI_URL##*/}" ../cwsdpmi
echo "vendor/: DJGPP, DOSBox Staging and CWSDPMI ready"
