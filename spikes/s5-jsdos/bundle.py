#!/usr/bin/env python3
# Writes a js-dos 8 bundle (.jsdos): a zip with .jsdos/dosbox.conf and .jsdos/jsdos.json beside the program's files.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: bundle.py --out OUT.jsdos [--set SECTION.KEY=VALUE]... [--run COMMAND]... FILE[=NAME]...
#
#   --out    the bundle to write
#   --set    a DOSBox setting that replaces the default below (for example --set cpu.cputype=486)
#   --run    a DOS command line for the [autoexec] section, after the mount; several are run in order
#   FILE     a file for the root of the emulated C: drive; FILE=NAME stores it under another name
#
# Every entry has the same time and permissions, so the same inputs give the same bundle.
import sys
import zipfile

# The settings for a program that runs in the browser. Everything not listed keeps js-dos's DOSBox default.
DEFAULTS = {
    ("sdl", "fullscreen"): "false",
    ("dosbox", "machine"): "svga_s3",
    ("dosbox", "memsize"): "16",
    ("cpu", "core"): "auto",
    ("cpu", "cputype"): "auto",
    ("cpu", "cycles"): "max",
    ("sblaster", "sbtype"): "sb2",
    ("sblaster", "sbbase"): "220",
    ("sblaster", "irq"): "7",
    ("sblaster", "dma"): "1",
    ("sblaster", "oplmode"): "opl2",
    ("sblaster", "oplemu"): "fast",
    ("mixer", "rate"): "44100",
    ("mixer", "blocksize"): "1024",
    ("mixer", "prebuffer"): "0",
}
STAMP = (2026, 10, 1, 0, 0, 0)


def main(argv):
    settings = dict(DEFAULTS)
    out = None
    runs = []
    rest = []
    it = iter(argv)
    for arg in it:
        if arg == "--set":
            key, _, value = next(it).partition("=")
            section, _, name = key.partition(".")
            settings[(section, name)] = value
        elif arg == "--out":
            out = next(it)
        elif arg == "--run":
            runs.append(next(it))
        else:
            rest.append(arg)
    files = rest
    if out is None or not files:
        sys.exit(__doc__)

    sections = {}
    for (section, name), value in settings.items():
        sections.setdefault(section, []).append(f"{name} = {value}")
    conf = ""
    for section, lines in sections.items():
        conf += f"[{section}]\n" + "\n".join(lines) + "\n\n"
    conf += "[autoexec]\nmount c .\nc:\n" + "".join(f"{r}\n" for r in runs)

    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        def add(name, data):
            info = zipfile.ZipInfo(name, STAMP)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)

        add(".jsdos/dosbox.conf", conf)
        add(".jsdos/jsdos.json", '{"version":"8.xx"}\n')
        for spec in files:
            path, _, name = spec.partition("=")
            with open(path, "rb") as f:
                add(name or path.rsplit("/", 1)[-1], f.read())


main(sys.argv[1:])
