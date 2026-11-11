#!/usr/bin/env python3
# Builds an editable OpenDocument presentation about Desk's DOS and native foundations.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/build-runtime-presentation.py [output.odp]
import sys
import zipfile
from pathlib import Path
from xml.etree import ElementTree as ET

NS = {
    'office': 'urn:oasis:names:tc:opendocument:xmlns:office:1.0',
    'style': 'urn:oasis:names:tc:opendocument:xmlns:style:1.0',
    'text': 'urn:oasis:names:tc:opendocument:xmlns:text:1.0',
    'draw': 'urn:oasis:names:tc:opendocument:xmlns:drawing:1.0',
    'fo': 'urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0',
    'svg': 'urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0',
    'presentation': 'urn:oasis:names:tc:opendocument:xmlns:presentation:1.0',
    'meta': 'urn:oasis:names:tc:opendocument:xmlns:meta:1.0',
    'dc': 'http://purl.org/dc/elements/1.1/',
    'manifest': 'urn:oasis:names:tc:opendocument:xmlns:manifest:1.0',
    'xlink': 'http://www.w3.org/1999/xlink',
}
for prefix, uri in NS.items():
    ET.register_namespace(prefix, uri)


def q(name):
    prefix, local = name.split(':')
    return f'{{{NS[prefix]}}}{local}'


def node(parent, tag, attrs=None, text=None):
    el = ET.SubElement(parent, q(tag), {q(k): str(v) for k, v in (attrs or {}).items()})
    el.text = text
    return el


def root(tag):
    return ET.Element(q(tag), {q('office:version'): '1.3'})


def xml(el):
    return ET.tostring(el, encoding='utf-8', xml_declaration=True)


BG = '#0c0c10'
PANEL = '#1b1e28'
INK = '#e0e0e0'
MUTED = '#b4bfce'
AMBER = '#ffaa40'
CYAN = '#22d3ee'
GREEN = '#2ec66a'
PURPLE = '#a78bfa'
WIDTH, HEIGHT = 34, 19.125

CONTENT = root('office:document-content')
AUTO = node(CONTENT, 'office:automatic-styles')
PAGE = node(AUTO, 'style:style', {'style:name': 'Page', 'style:family': 'drawing-page'})
node(PAGE, 'style:drawing-page-properties', {'draw:fill': 'solid', 'draw:fill-color': BG, 'presentation:background-visible': 'true'})
TEXTBOX = node(AUTO, 'style:style', {'style:name': 'Textbox', 'style:family': 'graphic'})
node(TEXTBOX, 'style:graphic-properties', {
    'draw:fill': 'none', 'draw:stroke': 'none', 'fo:padding': '0cm',
    'draw:auto-grow-height': 'false', 'draw:auto-grow-width': 'false',
    'draw:textarea-vertical-align': 'top',
})
BODY = node(CONTENT, 'office:body')
PRESENTATION = node(BODY, 'office:presentation')
STYLE_CACHE = {}
COUNTER = 0
SLIDES = []


def text_style(size, colour, bold=False, align='start'):
    key = (size, colour, bold, align)
    if key not in STYLE_CACHE:
        name = f'Text{len(STYLE_CACHE)}'
        st = node(AUTO, 'style:style', {'style:name': name, 'style:family': 'paragraph'})
        node(st, 'style:paragraph-properties', {
            'fo:text-align': align, 'fo:margin': '0cm', 'fo:line-height': '115%',
        })
        node(st, 'style:text-properties', {
            'fo:font-family': 'Liberation Sans', 'fo:font-size': f'{size}pt',
            'fo:color': colour, 'fo:font-weight': 'bold' if bold else 'normal',
        })
        STYLE_CACHE[key] = name
    return STYLE_CACHE[key]


def rect(page, x, y, w, h, colour):
    global COUNTER
    COUNTER += 1
    name = f'Fill{COUNTER}'
    st = node(AUTO, 'style:style', {'style:name': name, 'style:family': 'graphic'})
    node(st, 'style:graphic-properties', {'draw:fill': 'solid', 'draw:fill-color': colour, 'draw:stroke': 'none'})
    return node(page, 'draw:rect', {
        'draw:style-name': name, 'svg:x': f'{x}cm', 'svg:y': f'{y}cm',
        'svg:width': f'{w}cm', 'svg:height': f'{h}cm',
    })


def text(page, x, y, w, h, words, size=22, colour=INK, bold=False, align='start'):
    global COUNTER
    COUNTER += 1
    frame = node(page, 'draw:frame', {
        'draw:name': f'Textbox{COUNTER}', 'draw:style-name': 'Textbox',
        'svg:x': f'{x}cm', 'svg:y': f'{y}cm', 'svg:width': f'{w}cm', 'svg:height': f'{h}cm',
    })
    box = node(frame, 'draw:text-box')
    for line in words.split('\n'):
        node(box, 'text:p', {'text:style-name': text_style(size, colour, bold, align)}, line)


def slide(section, title, subtitle, sources, notes=''):
    index = len(SLIDES) + 1
    page = node(PRESENTATION, 'draw:page', {
        'draw:name': f'{index:02d} {section}', 'draw:style-name': 'Page', 'draw:master-page-name': 'Default',
    })
    rect(page, 1.45, 1.05, .18, .45, AMBER)
    text(page, 1.85, 1.04, 28, .6, section.upper(), 12, AMBER, True)
    text(page, 1.5, 2.05, 31, 1.35, title, 34, INK, True)
    text(page, 1.55, 3.62, 30.8, 1.2, subtitle, 18, MUTED)
    rect(page, 1.5, 17.65, 31, .025, '#333847')
    text(page, 1.55, 18.02, 27, .45, 'LIMINAL HQ  /  DESK  /  Architecture briefing  /  24 October 2026', 10, MUTED)
    text(page, 30.6, 17.94, 1.8, .65, f'{index:02d}', 14, AMBER, True, 'end')
    speaker = node(page, 'presentation:notes')
    frame = node(speaker, 'draw:frame', {
        'presentation:class': 'notes', 'draw:style-name': 'Textbox',
        'svg:x': '1cm', 'svg:y': '2cm', 'svg:width': '24cm', 'svg:height': '15cm',
    })
    box = node(frame, 'draw:text-box')
    node(box, 'text:p', text=notes)
    node(box, 'text:p', text='Repository sources: ' + '; '.join(sources))
    SLIDES.append(page)
    return page


def card(page, x, y, w, h, label, title, body, accent=AMBER):
    rect(page, x, y, w, h, PANEL)
    rect(page, x, y, w, .08, accent)
    text(page, x+.5, y+.45, w-1, .6, label.upper(), 11, accent, True)
    text(page, x+.5, y+1.3, w-1, 2.1, title, 24, INK, True)
    text(page, x+.5, y+3.65, w-1, h-3.9, body, 20, MUTED)


def band(page, x, y, w, label, colour=AMBER, h=1.35):
    rect(page, x, y, w, h, PANEL)
    rect(page, x, y, .10, h, colour)
    text(page, x+.3, y+.25, w-.6, h-.3, label, 19, INK, True, 'center')


def callout(page, words, colour=AMBER):
    text(page, 1.6, 15.95, 30.8, 1, words, 20, colour, True)


def build_slides():
    p = slide('Two foundations, one desktop', 'Rust on DOS. Rust on the machine.',
              'How the DOS runtime and native kernel support the same Desk shell.',
              ['docs/DIRECTION.md', 'docs/RUST-DOS-INTEGRATION-ANALYSIS.md'],
              'This deck explains the current implementation and proposed boundaries. Earlier emulator results are recorded evidence, not fresh runs for this presentation. No real-hardware claims are made.')
    text(p, 1.6, 5.35, 15, 3.6, 'One desktop.\nTwo ways to reach the PC.', 32, INK, True)
    text(p, 1.6, 10, 13.7, 3.3, 'DOS supplies an existing foundation.\nThe native kernel builds its own.\nShared Rust code sits above both.', 22, MUTED)
    band(p, 18, 5.4, 14.5, 'Desk shell + app services', PURPLE)
    text(p, 24.1, 7.05, 2, .8, '↓', 25, MUTED, align='center')
    band(p, 18, 8.15, 14.5, 'Shared contracts + drivers', PURPLE)
    text(p, 20.7, 9.8, 2, .8, '↓', 25, AMBER, align='center')
    text(p, 27.8, 9.8, 2, .8, '↓', 25, CYAN, align='center')
    band(p, 18, 10.9, 6.9, 'DOS runtime', AMBER)
    band(p, 25.6, 10.9, 6.9, 'Native kernel', CYAN)
    text(p, 18, 13.1, 6.9, 1.5, 'DOS + CWSDPMI', 18, AMBER, align='center')
    text(p, 25.6, 13.1, 6.9, 1.5, 'Hardware + BIOS', 18, CYAN, align='center')
    callout(p, 'Architecture and source assessment • implemented paths and future work distinguished')

    p = slide('The common ground', 'The shared shell already has two homes.',
              'Deterministic events and frames make the desktop portable and testable.',
              ['shell/shell/src/shell.rs', 'shell/dos/src/main.rs', 'kernel/src/shell.rs', 'docs/FIRST-WINDOW.md'],
              'The first-window result records matching DOS and native frames at both colour depths. This is evidence for the shared compositor/UI boundary, not proof of a complete OS or integrated app host.')
    band(p, 3, 5.4, 28, 'Events → window manager → compositor → presenter', PURPLE, 1.6)
    card(p, 1.5, 8.1, 15, 6.7, 'Hosted path', 'Desk inside DOS', 'Input and display wired through\nthe Rust DOS runtime.', AMBER)
    card(p, 17.5, 8.1, 15, 6.7, 'Native path', 'Desk inside the kernel', 'The same shell wired to native\ninput and display services.', CYAN)
    callout(p, 'Recorded result: matching first-window frames at 8-bit and 4-bit colour depths.', GREEN)

    p = slide('The DOS runtime', 'A foundation for standalone Rust programs.',
              'Startup, memory, DOS services and hardware access behind a small Rust interface.',
              ['spikes/d0-rust-dos/rt/src/lib.rs', 'docs/D0-RUST-DOS.md'],
              'The runtime is no_std with alloc. It replaces the C runtime for these executables, but still uses the loader stub and CWSDPMI. It currently lives under spikes/d0-rust-dos/rt.')
    card(p, 1.5, 5.5, 9.7, 9.5, '01 / Bring up', 'Make Rust runnable', 'Own stack and entry\nx87 initialisation\nHeap for Vec and String', AMBER)
    card(p, 12.15, 5.5, 9.7, 9.5, '02 / Serve', 'Reach DOS and devices', 'Files and console\nArguments and environment\nIRQ and display adapters', AMBER)
    card(p, 22.8, 5.5, 9.7, 9.5, '03 / Restore', 'Leave a usable machine', 'Run clean-up actions\nRestore vectors and video\nReturn an exit status', AMBER)
    callout(p, 'Reusable consumer: a DOS game or utility can use the runtime without Desk.')

    p = slide('Executable lifecycle', 'What happens before main()?',
              'The DOS path arrives in protected mode with help from a stub and DPMI host.',
              ['spikes/d0-rust-dos/rt/src/start.rs', 'spikes/d0-rust-dos/build.sh', 'tools/elf2go32.py'],
              'The source entry macro accepts fn main() -> i32; the program uses no_main rather than ordinary std lang_start. The DOS entry has a 256 KiB BSS stack. ELF linking and go32 packaging are distinct steps.')
    stages = [('01', 'Build', 'Rust + rust-lld\nresolve the ELF image'), ('02', 'Package', 'Packer adds the\nDOS loader stub'), ('03', 'Enter', 'Stub finds CWSDPMI\nand loads the image'), ('04', 'Run', 'Runtime sets up\nand calls Rust')]
    for i, (n, title, body) in enumerate(stages):
        card(p, 1.5+i*7.9, 5.6, 7.3, 7.8, n, title, body, AMBER)
    band(p, 3, 14.25, 28, 'Return or panic → clean-up → DOS exit status', AMBER)
    callout(p, 'No C library in this route; the stub and CWSDPMI remain dependencies.')

    p = slide('The address boundary', 'A Rust pointer is not a DOS address.',
              'Pointer-taking DOS calls cross through conventional memory.',
              ['spikes/d0-rust-dos/rt/src/dos.rs', 'spikes/d0-rust-dos/rt/src/dpmi.rs', 'docs/D0-RUST-DOS.md'],
              'D0 records a negative test: direct protected-mode INT 21h with a Rust buffer pointer reports a write count but does not write that buffer. Pointer-taking calls therefore use DPMI real-mode simulation. The DOS transfer buffer is 32 KiB; larger operations loop.')
    band(p, 2, 5.7, 9, 'Rust buffer', PURPLE, 1.8)
    band(p, 12.5, 5.7, 9, '32 KiB transfer buffer', AMBER, 1.8)
    band(p, 23, 5.7, 9, 'DOS file service', AMBER, 1.8)
    text(p, 10.9, 6, 1.5, .9, '→', 25, MUTED, align='center')
    text(p, 21.45, 6, 1.5, .9, '→', 25, MUTED, align='center')
    text(p, 2.2, 8.5, 8.6, 3, 'Protected-mode pointer\ninside the Rust program', 21, MUTED)
    text(p, 12.7, 8.5, 8.6, 3, 'Conventional memory\nDOS can address', 21, MUTED)
    text(p, 23.2, 8.5, 8.6, 3, 'Real-mode call\nmediated by DPMI', 21, MUTED)
    band(p, 3.5, 12.5, 27, 'Write: copy in → call DOS   /   Read: call DOS → copy out', AMBER, 1.7)
    callout(p, 'The runtime carries the translation; callers work with Rust slices.')

    p = slide('The native foundation', 'The kernel owns the environment.',
              'Without DOS underneath, the foundation must establish its own machine services.',
              ['kernel/src/main.rs', 'kernel/src/hal.rs', 'kernel/src/memory.rs', 'kernel/src/heap.rs', 'kernel/src/bios.rs'],
              'Current source establishes CPU tables, exceptions/IRQs, physical-memory accounting and a heap; it runs shared input/display and shell code. The BIOS trampoline exists. Filesystems, general app loading and stronger process isolation are not implied by these foundations.')
    card(p, 1.5, 5.5, 9.7, 9.5, '01 / Establish', 'Boot and CPU state', 'Boot information\nDescriptors and exceptions\nInterrupt controllers', CYAN)
    card(p, 12.15, 5.5, 9.7, 9.5, '02 / Manage', 'Memory and devices', 'Physical memory and heap\nTimer and IRQ dispatch\nShared hardware drivers', CYAN)
    card(p, 22.8, 5.5, 9.7, 9.5, '03 / Grow', 'System services ahead', 'Storage and filesystem\nApp loading and scheduling\nShutdown and recovery policy', CYAN)
    callout(p, 'A booting kernel is the foundation; a complete operating system adds durable services.', CYAN)

    p = slide('Responsibilities compared', 'Similar services. Different obligations.',
              'The native foundation takes ownership of services DOS already supplies.',
              ['spikes/d0-rust-dos/rt/src/lib.rs', 'kernel/src/main.rs', 'docs/DIRECTION.md'],
              'The table mixes implemented services and explicit future work, as labelled. Native BIOS calls and disk probes do not amount to a complete filesystem. Shutdown behaviour is a product decision, beyond emulator exit helpers.')
    cols = [(1.5, 7.0), (8.65, 11.5), (20.3, 12.2)]
    for (x, w), label, c in zip(cols, ['SERVICE', 'DOS RUNTIME', 'NATIVE FOUNDATION'], [MUTED, AMBER, CYAN]):
        text(p, x+.2, 5.3, w-.4, .7, label, 13, c, True)
    rows = [('Startup', 'Stub + DPMI host', 'Bootloader + kernel entry'), ('Memory', 'Request host memory blocks', 'Manage physical memory'), ('Interrupts', 'Install through DPMI', 'Own tables and dispatch'), ('BIOS access', 'DPMI real-mode simulation', 'Kernel real-mode trampoline'), ('Files', 'Call DOS file services', 'Filesystem remains future work'), ('Exit', 'Restore state; return to DOS', 'Define system shutdown/restart')]
    for i, row in enumerate(rows):
        y = 6.4 + i*1.47
        if i % 2 == 0:
            rect(p, 1.5, y-.08, 31, 1.36, PANEL)
        for (x, w), words in zip(cols, row):
            text(p, x+.2, y+.12, w-.4, .95, words, 18, INK)
    callout(p, 'The contracts can be shared while resource ownership stays platform-specific.', PURPLE)

    p = slide('Sharing the right code', 'Share algorithms. Respect machine lifetimes.',
              'The HAL gives shared drivers a small surface to work against.',
              ['hal/hal/src/video.rs', 'hal/hal/src/irq.rs', 'kernel/src/hal.rs', 'docs/HAL.md'],
              'Shared examples include VGA/PIC/PIT/keyboard code. Platform-specific differences include vector installation, interrupt chaining, memory acquisition and mapping lifetime. Current cleanup and framebuffer-lifetime gaps are detailed in the analysis report.')
    card(p, 1.5, 5.6, 15, 8.9, 'Shared code', 'Behaviour and device algorithms', 'Keyboard decoding\nVGA register programming\nPalette and presentation\nCompositor and window behaviour', PURPLE)
    card(p, 17.5, 5.6, 15, 8.9, 'Platform code', 'Acquisition and restoration', 'How memory is obtained\nHow an IRQ becomes live\nHow firmware calls execute\nWhen mappings become invalid', CYAN)
    callout(p, 'A safe interface must preserve the lifetime promised by its unsafe implementation.', PURPLE)

    p = slide('Two meanings of runtime', 'A Desk app needs a thinner runtime.',
              'A standalone executable and a module inside Desk have different owners.',
              ['spikes/d0-rust-dos/rt/src/lib.rs', 'spikes/s1-dxe3/modules/rt/src/lib.rs', 'docs/SDK.md', 'docs/S1-DXE3.md'],
              'The app runtime is not the D0 platform runtime. A module forwards allocation and services to the host, exposes entry points and reports panics. The experimental std ABI and app SDK currently overlap in names but differ in signatures; they must be reconciled before composition.')
    card(p, 1.5, 5.6, 15, 9.2, 'Standalone program', 'Platform runtime', 'Own executable startup\nOwn heap and DOS service access\nOwn hardware restoration\nExit back to DOS', AMBER)
    card(p, 17.5, 5.6, 15, 9.2, 'Loadable Desk app', 'App runtime + SDK', 'Host calls app entry points\nHost tracks app allocations\nApp uses host services\nHost manages stop and unload', PURPLE)
    callout(p, 'Per-module heaps and hardware hooks would create competing ownership.')

    p = slide('Evidence and remaining work', 'The pieces work. Their composition is next.',
              'Keep demonstrated paths distinct from the product promises still ahead.',
              ['tools/check.sh', 'docs/FIRST-WINDOW.md', 'docs/S1-DXE3.md', 'docs/E-STD-STUBS.md', 'docs/HANDOFF.md'],
              'Recorded emulator results have not been rerun to make this deck. The separate DXE3 host demonstrates selected recovery but not isolation: stack overflow, in-address-space corruption and infinite loops remain limits. No real-hardware or audible-output verification is claimed.')
    card(p, 1.5, 5.6, 15, 9.1, 'Implemented / recorded', 'Useful foundations', 'Rust-only DOS executables\nShared shell on both tracks\nMatching first-window frames\nDXE3 lifecycle in a separate host', GREEN)
    card(p, 17.5, 5.6, 15, 9.1, 'Not yet combined / planned', 'A dependable app system', 'Real app in the Rust-only shell\nUnified, versioned SDK contract\nReliable launcher and resume\nNative storage and app services', AMBER)
    callout(p, 'Recovered panics are useful; trusted modules still share the host’s failure domain.')

    p = slide('Repository boundaries', 'Separate the shell from its foundations.',
              'A suggested medium-term split; these repositories have not been created.',
              ['docs/REPOSITORY-BREAKUP-PROPOSAL.md'],
              'This is Option 2 of the breakup proposal. pc-hal keeps drivers and mocks with the contracts. desk-system temporarily combines kernel ownership and full-system assembly. The full layer-cake option later separates kernel, drivers, SDK and apps. Names are working names.')
    items = [('desk', 'Desktop, SDK, apps and adapters', PURPLE), ('pc-hal', 'Shared contracts, drivers and mocks', GREEN), ('dos-rs', 'DOS runtime and executable tooling', AMBER), ('desk-system', 'Native kernel + product assembly', CYAN)]
    for i, (name, purpose, accent) in enumerate(items):
        y = 5.5+i*2.3
        band(p, 1.5, y, 8, name, accent, 1.75)
        text(p, 10.5, y+.25, 21.8, 1.2, purpose, 23, INK)
    callout(p, 'First prove independent consumers; then move ownership and pin dependencies.', PURPLE)

    p = slide('The next architectural proof', 'Run one real app in the shared shell.',
              'Connect the runtime, SDK and app host through a complete lifecycle.',
              ['docs/RUST-DOS-INTEGRATION-ANALYSIS.md', 'docs/REPOSITORY-BREAKUP-PROPOSAL.md'],
              'This is the recommended next integration milestone, not a completed feature. Before it, address fallible cleanup registration and framebuffer mapping lifetimes, then audit non-local recovery across Rust/C callback frames. Keep the old C/DJGPP services until replacement scenarios pass.')
    for i, (title, body) in enumerate([
        ('Own resources', 'Enforce cleanup and\nframebuffer lifetimes'),
        ('Load and call', 'Bind a real SDK service\nand deliver shell events'),
        ('Stop and recover', 'Unload repeatedly; keep\nanother app usable'),
    ]):
        card(p, 1.5+i*10.65, 5.6, 9.7, 8.9, f'0{i+1}', title, body, [AMBER, PURPLE, CYAN][i])
    callout(p, 'Success: one app starts, draws, receives events and unloads without destabilising Desk.', GREEN)

    p = slide('Source trail and scope', 'The code is the source of truth.',
              'Repository references and verification limits for this briefing.',
              ['docs/DIRECTION.md', 'docs/D0-RUST-DOS.md', 'docs/HAL.md', 'docs/FIRST-WINDOW.md', 'docs/S1-DXE3.md', 'docs/E-STD-STUBS.md', 'docs/RUST-DOS-INTEGRATION-ANALYSIS.md', 'docs/REPOSITORY-BREAKUP-PROPOSAL.md'],
              'Paths are relative to the liminal-hq/desk repository. Detailed per-slide references are in speaker notes. Presentation render validation does not rerun any DOS/native correctness or performance tests. Deck generated from editable native ODF shapes and text.')
    text(p, 1.6, 5.5, 15, .8, 'IMPLEMENTATION AND RESULTS', 13, PURPLE, True)
    text(p, 1.6, 6.75, 15, 6.8, 'docs/D0-RUST-DOS.md\ndocs/HAL.md\ndocs/FIRST-WINDOW.md\ndocs/S1-DXE3.md\ndocs/E-STD-STUBS.md', 20, MUTED)
    text(p, 18, 5.5, 14.5, .8, 'ASSESSMENT AND PROPOSAL', 13, AMBER, True)
    text(p, 18, 6.75, 14.5, 4, 'docs/\nRUST-DOS-INTEGRATION-ANALYSIS.md\n\ndocs/\nREPOSITORY-BREAKUP-PROPOSAL.md', 16, MUTED)
    text(p, 18, 12.15, 14.5, 2.4, 'No fresh DOS/native test runs.\nNo hardware timing claims.\nPer-slide sources in speaker notes.', 20, INK)
    callout(p, 'Two foundations can support one desktop when the service contracts remain clear.', PURPLE)


def write_deck(destination, title='Rust on DOS. Rust on the machine.',
               subject='Desk: DOS runtime, native kernel and shared architecture', assets=None):
    assets = assets or {}
    styles = root('office:document-styles')
    node(styles, 'office:styles')
    auto = node(styles, 'office:automatic-styles')
    layout = node(auto, 'style:page-layout', {'style:name': 'Wide'})
    node(layout, 'style:page-layout-properties', {
        'fo:page-width': f'{WIDTH}cm', 'fo:page-height': f'{HEIGHT}cm',
        'style:print-orientation': 'landscape', 'fo:margin': '0cm',
    })
    masters = node(styles, 'office:master-styles')
    node(masters, 'style:master-page', {'style:name': 'Default', 'style:page-layout-name': 'Wide'})
    meta = root('office:document-meta')
    data = node(meta, 'office:meta')
    node(data, 'dc:title', text=title)
    node(data, 'dc:subject', text=subject)
    node(data, 'dc:creator', text='Liminal HQ')
    node(data, 'dc:language', text='en-CA')
    node(data, 'meta:generator', text='Desk runtime presentation builder')
    manifest = ET.Element(q('manifest:manifest'), {q('manifest:version'): '1.3'})
    mime = 'application/vnd.oasis.opendocument.presentation'
    for name, kind in [('/', mime), ('content.xml', 'text/xml'), ('styles.xml', 'text/xml'), ('meta.xml', 'text/xml')]:
        node(manifest, 'manifest:file-entry', {'manifest:full-path': name, 'manifest:media-type': kind})
    for name, (kind, _) in assets.items():
        node(manifest, 'manifest:file-entry', {'manifest:full-path': name, 'manifest:media-type': kind})
    destination.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(destination, 'w') as archive:
        archive.writestr('mimetype', mime, compress_type=zipfile.ZIP_STORED)
        for name, document in [('content.xml', CONTENT), ('styles.xml', styles), ('meta.xml', meta), ('META-INF/manifest.xml', manifest)]:
            archive.writestr(name, xml(document), compress_type=zipfile.ZIP_DEFLATED)
        for name, (_, payload) in assets.items():
            archive.writestr(name, payload, compress_type=zipfile.ZIP_DEFLATED)
    print(f'{destination}: {len(SLIDES)} editable slides')


if __name__ == '__main__':
    build_slides()
    write_deck(Path(sys.argv[1] if len(sys.argv) > 1 else 'dist/presentations/rust-dos-native-runtime.odp'))
