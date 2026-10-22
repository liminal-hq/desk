#!/bin/bash
# Validates the built fonts with third-party tools: fc-scan and otfinfo on the host if present, then fontTools, OTS
# and FreeType in a throwaway Docker image. Optional and not part of tools/check.sh; nothing is installed on the host.
# Set PROOF_DIR to a directory to also get FreeType proof sheets of the outline faces.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: fonts/validate.sh
set -euo pipefail
cd "$(dirname "$0")"
IMAGE=desk-fonts-validate:local

if command -v fc-scan >/dev/null; then
  echo "== fc-scan"
  fc-scan --format '%{file}: %{family} | %{style} | %{fullname} | %{fontformat} | spacing %{spacing:-proportional}\n' generated/*.ttf generated/*.otf
fi
if command -v otfinfo >/dev/null; then
  echo; echo "== otfinfo -i (TrueType files)"
  for f in generated/*.ttf; do echo "-- $f"; otfinfo -i "$f" | grep -E 'Family|Subfamily|Full name|PostScript|Permissions'; done
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker is not available: skipping fontTools, OTS and FreeType" >&2
  exit 0
fi
# A small image with pinned tools, built once and cached; the Dockerfile is here so nothing else is needed.
docker build -q -t "$IMAGE" - >/dev/null <<'EOF'
FROM python:3.12-slim
RUN pip install --no-cache-dir fonttools==4.55.3 opentype-sanitizer==9.2.0 freetype-py==2.5.1
EOF
echo
# PROOF_DIR=<dir> also writes FreeType proof sheets of the outline faces there.
if [ -n "${PROOF_DIR:-}" ]; then
  mkdir -p "$PROOF_DIR"
  docker run --rm --network none --user "$(id -u):$(id -g)" \
    -v "$PWD/generated:/fonts:ro" -v "$PWD/validate.py:/validate.py:ro" -v "$(cd "$PROOF_DIR" && pwd):/proof" \
    "$IMAGE" python3 /validate.py /fonts /proof
else
  docker run --rm --network none \
    -v "$PWD/generated:/fonts:ro" -v "$PWD/validate.py:/validate.py:ro" \
    "$IMAGE" python3 /validate.py /fonts
fi
