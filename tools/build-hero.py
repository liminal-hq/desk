#!/usr/bin/env python3
# Builds assets/hero.svg, the README banner: a pixel-art desktop (windows, a launcher bar, a horizon) beside the project name.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/build-hero.py [name] > assets/hero.svg
#
# The name defaults to the working name below; pass another to regenerate the banner after the product is renamed. Colours are
# the Afterglow ramps (liminal-hq/afterglow-themes, dark) and the palette roles in docs/PALETTE.md. The motion (a drifting window,
# a pulsing focus halo, a moving pointer and a blinking caret) is switched off by prefers-reduced-motion.
import sys

NAME = sys.argv[1] if len(sys.argv) > 1 else "Desk"
TAGLINE = "A calm graphical shell for MS-DOS"
PROMPT = "C:\\&gt; DESK.COM"
FACTS = "VESA 640×480 · 256 colours · Rust + DJGPP C"

# The horizon: seven stepped bands, purple through red to orange, never a smooth gradient (the desktop has no alpha).
HORIZON = ["#2d2747", "#413764", "#5b1e30", "#7d263b", "#a22d46", "#cb3652", "#d48e39"]
MONO = "'JetBrains Mono', 'DejaVu Sans Mono', Menlo, Consolas, monospace"
SANS = "'Space Grotesk', 'Inter', 'Helvetica Neue', Arial, sans-serif"


def rect(x, y, w, h, fill, extra=""):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"{extra}/>'


def bars(x, y, widths, fill, step=9, h=3):
    """Skeleton text lines: one bar per width, standing in for text at this size."""
    return "".join(rect(x, y + i * step, w, h, fill) for i, w in enumerate(widths))


def window(x, y, w, h, title, body, focused=False, cls=""):
    out = [f'<g transform="translate({x} {y})"{f" class=" + chr(34) + cls + chr(34) if cls else ""}>']
    if focused:
        # The stepped halo: three palette steps of orange around the window, a glow without alpha.
        out.append(rect(-6, -6, w + 12, h + 12, "#372612", ' class="halo"'))
        out.append(rect(-4, -4, w + 8, h + 8, "#7d5422"))
        out.append(rect(-2, -2, w + 4, h + 4, "#ffaa40"))
    else:
        out.append(rect(-1, -1, w + 2, h + 2, "#262a37"))
    out.append(rect(0, 0, w, h, "#111116"))
    out.append(rect(0, 0, w, 16, "#0c0c10"))
    out.append(rect(0, 16, w, 1, "#262a37"))
    out.append(f'<text x="8" y="12" font-family="{SANS}" font-size="10" font-weight="700" fill="{"#ffaa40" if focused else "#7c8796"}">{title}</text>')
    for i in range(3):
        out.append(rect(w - 40 + i * 12, 5, 8, 6, "#333847"))
    out.append(body)
    out.append("</g>")
    return "".join(out)


def desktop():
    parts = []
    # The screen and its bezel.
    parts.append(rect(-10, -10, 500, 270, "#1b1e28", ' rx="6"'))
    parts.append(rect(0, 0, 480, 250, "#050507"))
    for sx, sy in ((30, 14), (120, 8), (210, 22), (300, 10), (380, 30), (450, 16), (70, 40), (440, 56)):
        parts.append(rect(sx, sy, 2, 2, "#5f5d78"))
    for i, colour in enumerate(HORIZON):
        parts.append(rect(0, 142 + i * 12, 480, 12, colour))

    # Desktop icons: tiles with a glyph block and a label bar.
    for iy, glyph in ((18, "#ffaa40"), (72, "#a78bfa"), (126, "#22d3ee")):
        parts.append(rect(430, iy, 32, 32, "#1b1e28"))
        parts.append(rect(438, iy + 8, 16, 16, glyph))
        parts.append(rect(434, iy + 38, 24, 3, "#b8b6c8"))

    # A Files window behind, unfocused.
    files_body = (
        rect(0, 17, 54, 103, "#0c0c10")
        + bars(8, 26, (34, 28, 38, 24, 32), "#3a384f")
        + rect(54, 17, 1, 103, "#262a37")
        + bars(64, 28, (96, 120, 80, 108, 88, 112), "#2a2840")
        + rect(64, 26, 70, 7, "#372612")
        + rect(64, 26, 2, 7, "#ffaa40")
    )
    parts.append(window(22, 22, 226, 120, "Files", files_body))

    # The focused Terminal window in front, drifting a little.
    term_body = (
        rect(0, 17, 244, 107, "#050507")
        + f'<text x="10" y="38" font-family="{MONO}" font-size="11" fill="#2ec66a">C:\\&gt; DIR</text>'
        + bars(10, 46, (150, 118, 164), "#3a384f", step=10, h=3)
        + f'<text x="10" y="90" font-family="{MONO}" font-size="11" fill="#2ec66a">C:\\&gt; KEEN4</text>'
        + f'<text x="10" y="108" font-family="{MONO}" font-size="11" fill="#2ec66a">C:\\&gt; </text>'
        + rect(52, 99, 7, 11, "#ffaa40", ' class="caret"')
    )
    parts.append(window(158, 56, 244, 124, "Terminal", term_body, focused=True, cls="drift"))

    # The launcher bar: Desk menu, one button per window, the clock.
    parts.append(rect(0, 226, 480, 24, "#1b1e28"))
    parts.append(rect(0, 226, 480, 1, "#333847"))
    parts.append(rect(4, 230, 44, 16, "#ffaa40"))
    parts.append(f'<text x="12" y="242" font-family="{SANS}" font-size="11" font-weight="700" fill="#050507">{NAME}</text>')
    parts.append(rect(56, 230, 60, 16, "#262a37"))
    parts.append(rect(122, 230, 60, 16, "#372612"))
    parts.append(rect(122, 244, 60, 2, "#ffaa40"))
    parts.append(f'<text x="130" y="242" font-family="{SANS}" font-size="10" fill="#e4e2ec">Terminal</text>')
    parts.append(f'<text x="64" y="242" font-family="{SANS}" font-size="10" fill="#8f8da6">Files</text>')
    parts.append(f'<text x="428" y="242" font-family="{SANS}" font-size="10" fill="#b8b6c8">Thu 22 Oct · 3:06</text>')

    # The pointer: a stepped pixel arrow, drawn by the desktop itself.
    arrow = "M0 0h2v2h-2zM0 2h2v2h-2zM0 4h4v2h-4zM0 6h6v2h-6zM0 8h8v2h-8zM0 10h10v2h-10zM0 12h4v2h-4zM6 12h2v2h-2zM6 14h2v2h-2z"
    parts.append(f'<g class="pointer"><path d="{arrow}" fill="#050507" transform="translate(-1 -1) scale(1.2)"/><path d="{arrow}" fill="#f1eee6"/></g>')
    return "".join(parts)


svg = f"""<!-- Hero banner for the README: a pixel-art desktop beside the project name. Built by tools/build-hero.py. -->
<!-- -->
<!-- (c) Copyright 2026 Liminal HQ, Scott Morris -->
<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->
<svg width="1200" height="320" viewBox="0 0 1200 320" xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="hero-title hero-desc">
  <title id="hero-title">{NAME}</title>
  <desc id="hero-desc">{NAME}: {TAGLINE}. A dark banner showing a pixel-art desktop on a stepped sunset horizon, with a Files window behind a focused Terminal window wearing an orange halo, a launcher bar, and a pointer. Beside it, the name, a DOS prompt and the stack: VESA 640 by 480 in 256 colours, Rust and DJGPP C.</desc>

  <style>
    .drift {{ animation: drift 6s ease-in-out infinite; }}
    .halo {{ animation: glow 3s ease-in-out infinite; }}
    .pointer {{ animation: roam 9s ease-in-out infinite; }}
    .caret {{ animation: blink 1.1s steps(1) infinite; }}
    .type {{ animation: blink 1.1s steps(1) infinite; }}
    @keyframes drift {{ 0%, 100% {{ transform: translate(158px, 56px); }} 50% {{ transform: translate(158px, 52px); }} }}
    @keyframes glow {{ 0%, 100% {{ opacity: 1; }} 50% {{ opacity: 0.45; }} }}
    @keyframes roam {{ 0%, 100% {{ transform: translate(300px, 150px); }} 35% {{ transform: translate(330px, 100px); }} 65% {{ transform: translate(60px, 236px); }} 80% {{ transform: translate(60px, 236px); }} }}
    @keyframes blink {{ 0%, 55% {{ opacity: 1; }} 56%, 100% {{ opacity: 0; }} }}
    @media (prefers-reduced-motion: reduce) {{
      .drift, .halo, .pointer, .caret, .type {{ animation: none; }}
      .drift {{ transform: translate(158px, 56px); }}
      .pointer {{ transform: translate(300px, 150px); }}
    }}
  </style>

  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#050507"/>
      <stop offset="100%" stop-color="#15121f"/>
    </linearGradient>
    <!-- House hero-text gradient: white into Afterglow orange and rose. -->
    <linearGradient id="brand" x1="0" y1="0" x2="1" y2="0">
      <stop offset="20%" stop-color="#ffffff"/>
      <stop offset="65%" stop-color="#ffaa40"/>
      <stop offset="100%" stop-color="#f43f5e"/>
    </linearGradient>
  </defs>

  <rect width="1200" height="320" fill="url(#bg)"/>

  <g transform="translate(50 35)" shape-rendering="crispEdges">{desktop()}</g>

  <text x="640" y="140" font-family="{SANS}" font-size="104" font-weight="700" fill="url(#brand)">{NAME}</text>
  <text x="642" y="186" font-family="{MONO}" font-size="24" fill="#ffffff" fill-opacity="0.85">{TAGLINE}</text>
  <g font-family="{MONO}" font-size="22">
    <text x="642" y="238" fill="#aaaaaa">{PROMPT}</text>
    <rect class="type" x="{642 + 13 * 13.2:.0f}" y="221" width="12" height="22" fill="#ffaa40"/>
    <text x="642" y="276" font-size="18" fill="#22d3ee" fill-opacity="0.9">{FACTS}</text>
  </g>
</svg>
"""
sys.stdout.write(svg)
