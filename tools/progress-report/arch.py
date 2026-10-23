# The architecture diagram as inline SVG: layers of the two tracks, drawn solid where merged work exists and dashed where planned.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The boxes follow docs/DIRECTION.md. Colours come from classes in site.css, so the drawing follows the page's theme.
from mdlite import esc

RH = 50
YS = [48, 112, 176, 240, 304, 368, 432]
HX, HW, NX, NW = 150, 400, 570, 400
ROWS = ["Apps", "Core", "HAL", "Drivers", "Runtime / kernel", "Boot", "Machine"]
W, H = 980, 520

# (x, y, w, h, kind, line1, line2); kind is built (merged, runs in an emulator), plan (not started) or ext (not ours).
BOXES = [
    (HX, YS[0], HW + 20 + NW, RH, "plan", "Apps as modules and the SDK", "desk_* ABI shared by both tracks"),
    (HX + 12, YS[0] + 8, 190, RH - 16, "built", "DXE3 loading proven", "PR #4, with limits"),
    (HX, YS[1], 600, RH, "plan", "Desk core", "compositor · windows · widgets · event loop · app host"),
    (HX + 620, YS[1], NX + NW - HX - 620, RH, "built", "Fonts · emoji · palette", "PRs #1 and #2"),
    (HX, YS[2], HW, RH, "plan", "Hosted HAL", "over DOS, BIOS, VBE, INT 33h"),
    (NX, YS[2], NW, RH, "plan", "Native HAL", "over our own drivers"),
    (HX, YS[3], HW + 20 + NW, RH, "plan", "Shared Rust drivers", "VGA/VBE · keyboard · mouse · PIT · OPL · Sound Blaster"),
    (HX, YS[4], 192, RH, "built", "C layer + DJGPP libc", "PR #3 (to retire)"),
    (HX + 208, YS[4], 192, RH, "built", "rt: pure Rust runtime", "PR #7"),
    (NX, YS[4], NW, RH, "built", "Rust kernel", "GDT · IDT · PIC · PIT · frames · heap · x87 (PR #6)"),
    (HX, YS[5], HW, RH, "ext", "DOS and CWSDPMI", "not ours; DESK.EXE started from the prompt"),
    (NX, YS[5], NW, RH, "built", "NASM boot sector and loader", "INT 13h · A20 · E820 · GDT · protected mode (PR #6)"),
    (HX, YS[6], HW, RH, "ext", "486 PC, emulated", "DOSBox Staging, headless, in CI"),
    (NX, YS[6], NW, RH, "ext", "486 PC, emulated", "QEMU -cpu 486, headless, in CI"),
]

LABEL = ("Architecture of the two tracks. Planned and shared: apps, the Desk core, the HAL and the shared drivers. Merged and run in "
         "an emulator: the C layer and the pure Rust runtime on the hosted side, the Rust kernel and the boot sector on the native "
         "side, and the fonts, emoji and palette. Nothing has run on real hardware.")


def svg():
    o = ['<svg viewBox="0 0 %d %d" role="img" aria-label="%s" class="diagram arch">' % (W, H, esc(LABEL)),
         '<defs><marker id="arch-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
         '<path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>',
         '<text x="%d" y="26" text-anchor="middle" font-size="14" class="d-title">Hosted track · Desk on DOS</text>' % (HX + HW / 2),
         '<text x="%d" y="26" text-anchor="middle" font-size="14" class="d-title">Native track · own kernel</text>' % (NX + NW / 2)]
    for y, label in zip(YS, ROWS):
        o.append('<text x="0" y="%d" font-size="12" class="d-muted">%s</text>' % (y + RH / 2 + 4, esc(label)))
    for x, y, w, h, kind, l1, l2 in BOXES:
        dash = ' stroke-dasharray="6 4"' if kind == "plan" else ""
        o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (x, y, w, h, kind, dash))
        cx = x + w / 2
        if l2:
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="13" class="a-t1 a-t1-%s">%s</text>' % (cx, y + h / 2 - 3, kind, esc(l1)))
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="11.5" class="a-t2">%s</text>' % (cx, y + h / 2 + 13, esc(l2)))
        else:
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="13" class="a-t1 a-t1-%s">%s</text>' % (cx, y + h / 2 + 4, kind, esc(l1)))

    def arrow(x1, y1, x2, y2, label=""):
        o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.3" marker-end="url(#arch-arrow)"/>' % (x1, y1, x2, y2))
        if label:
            o.append('<text x="%g" y="%g" font-size="11" class="d-muted">%s</text>' % ((x1 + x2) / 2 + 6, (y1 + y2) / 2 + 4, esc(label)))

    arrow(HX + 620, YS[1] + RH / 2, HX + 602, YS[1] + RH / 2)
    arrow(HX + HW / 2, YS[2], HX + HW / 2, YS[1] + RH + 2, "calls only the HAL")
    arrow(NX + 80, YS[2], NX + 80, YS[1] + RH + 2)
    o.append('<text x="%d" y="%d" font-size="12" class="d-text2">Solid: merged on main and run in an emulator. Dashed: planned. Plain: not ours, or the machine.</text>' % (HX, H - 14))
    o.append('<text x="%d" y="%d" text-anchor="end" font-size="12" class="d-warn">Real hardware: not yet tried</text>' % (NX + NW, H - 14))
    o.append("</svg>")
    return "\n".join(o)
