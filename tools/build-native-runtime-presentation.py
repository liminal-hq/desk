#!/usr/bin/env python3
# Builds an editable presentation explaining Desk's native Rust kernel and platform services.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/build-native-runtime-presentation.py [output.odp]
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('runtime_deck', ROOT / 'tools/build-runtime-presentation.py')
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)
ASSETS = {}


def slide(section, title, subtitle, sources, notes):
    return d.slide(section, title, subtitle, sources, notes)


def cards(page, items, accent=d.CYAN):
    for i, (label, title, body) in enumerate(items):
        d.card(page, 1.5+i*10.65, 5.6, 9.7, 9.2, label, title, body, accent)


def picture(page, path, x, y, w, h):
    name = 'Pictures/' + path.name
    ASSETS[name] = ('image/png', path.read_bytes())
    frame = d.node(page, 'draw:frame', {
        'draw:name': 'Native shell screenshot', 'draw:style-name': 'Textbox',
        'svg:x': f'{x}cm', 'svg:y': f'{y}cm', 'svg:width': f'{w}cm', 'svg:height': f'{h}cm',
    })
    d.node(frame, 'draw:image', {
        'xlink:href': name, 'xlink:type': 'simple', 'xlink:show': 'embed', 'xlink:actuate': 'onLoad',
    })


def build():
    p = slide('The native Rust foundation', 'From the first instruction to Desk.',
              'Boot, memory, interrupts and devices on a 486-class PC.',
              ['docs/DIRECTION.md', 'docs/NATIVE-BOOT.md', 'kernel/src/main.rs'],
              'Native runtime here means the kernel startup and platform services underneath Desk. This is a focused architectural briefing, not a claim that a complete native OS or public runtime SDK exists. Earlier emulator evidence is attributed; no native runs were repeated to produce the deck.')
    d.text(p, 1.6, 5.6, 15.5, 3.2, 'The kernel builds\nthe environment Rust needs.', 31, d.INK, True)
    d.text(p, 1.6, 10.0, 14.5, 3.8, 'A shared shell above.\nMachine ownership below.\nExplicit contracts in between.', 23, d.MUTED)
    for y, label, accent in [(5.6, 'Desk shell', d.PURPLE), (8.0, 'HAL contracts + shared drivers', d.GREEN), (10.4, 'Native kernel services', d.CYAN), (12.8, 'Bootloader + PC hardware + BIOS', d.CYAN)]:
        d.band(p, 18, y, 14.5, label, accent, 1.5)
    d.callout(p, 'Working foundation today; durable system and app services still ahead.', d.CYAN)

    p = slide('Scope and ownership', 'A runtime grows into a kernel here.',
              'There is no DOS host to supply memory, interrupts or a return path.',
              ['kernel/src/main.rs', 'kernel/src/hal.rs', 'kernel/src/exit.rs', 'docs/DIRECTION.md'],
              'The kernel is currently a standalone executable. It supplies low-level services directly and imports the shared shell. Independent kernel packaging and a reusable native runtime API are proposed boundaries, not existing products. Rust std is not provided by this kernel.')
    cards(p, [('Environment', 'Make Rust runnable', 'Stack and static data\nHeap for alloc\nCPU and x87 setup'),
              ('Machine', 'Own the hardware', 'Physical memory\nInterrupts and timer\nDevice access and firmware'),
              ('System', 'Provide services', 'Input and display today\nStorage and apps ahead\nRecovery policy to define')])
    d.callout(p, 'no_std + alloc supplies language facilities; the kernel supplies the machine.', d.CYAN)

    p = slide('The boot chain', 'Firmware hands over one stage at a time.',
              'The loader gathers what Rust cannot safely assume at entry.',
              ['boot/stage1.asm', 'boot/stage2.asm', 'docs/NATIVE-BOOT.md'],
              'BIOS starts stage 1 at 0000:7C00. Stage 1 reads stage 2 and stages the kernel at physical 0x10000. Stage 2 opens A20, collects a memory map with E820/E801/AH=88 fallbacks, prepares boot information and optional VBE, then enters protected mode. It copies the kernel to 0x100000. The diagram is conceptual; source scripts define exact layouts.')
    stages = [('01', 'BIOS', 'Load the boot sector\nat 0000:7C00'), ('02', 'Stage 1', 'Read stage 2\nand kernel sectors'), ('03', 'Stage 2', 'A20, memory map\nand boot information'), ('04', 'Kernel', '32-bit entry\nat physical 1 MiB')]
    for i, (label, title, body) in enumerate(stages):
        d.card(p, 1.5+i*7.9, 5.6, 7.3, 8.4, label, title, body, d.CYAN)
    d.band(p, 3, 14.55, 28, 'Real mode: BIOS services → protected mode: kernel ownership', d.CYAN)
    d.callout(p, 'Boot information carries memory, video and firmware facts across the boundary.', d.PURPLE)

    p = slide('Entering Rust', 'Establish invariants before enabling IRQs.',
              'The current entry code defines the first safe environment for kernel code.',
              ['kernel/src/main.rs', 'kernel/src/gdt.rs', 'kernel/src/idt.rs', 'kernel/src/fpu.rs'],
              'Assembly startup disables interrupts, clears the direction flag, establishes a 64 KiB BSS stack, zeroes BSS and calls kmain with the loader boot-information pointer. kmain sets up serial/console, descriptor tables, PIC/PIT, memory and BIOS support before timer and input demonstrations. The x87 is initialised before its demonstration; this is not a statement that it runs before every earlier step.')
    d.card(p, 1.5, 5.6, 15, 9.3, 'Assembly entry', 'Control stack and state', 'Interrupts off; direction flag clear\nSelect the kernel stack\nZero uninitialised static storage\nCall kmain(boot_info)', d.CYAN)
    d.card(p, 17.5, 5.6, 15, 9.3, 'Rust bring-up', 'Make the machine observable', 'Start serial and text output\nCheck boot information\nInstall CPU and interrupt tables\nInitialise memory and devices', d.PURPLE)
    d.callout(p, 'Own stack, own handlers, known state: startup is a contract with the loader.', d.CYAN)

    p = slide('Memory ownership', 'Physical frames feed the Rust heap.',
              'The present kernel uses flat addressing with paging disabled.',
              ['kernel/src/memory.rs', 'kernel/src/heap.rs', 'docs/NATIVE-BOOT.md', 'docs/N3-BIOS-PATH.md'],
              'The frame bitmap covers the first 256 MiB, with 4 KiB frames. Available regions are freed and reserved regions take precedence; the first MiB and kernel image including BSS/stack stay reserved. Heap initialization requests 1024 contiguous frames, or 4 MiB. The heap is a first-fit, address-sorted coalescing free list; this is an implementation size, not a minimum-memory guarantee.')
    for i, (label, body) in enumerate([('BIOS memory map', 'Usable regions and reserved holes'), ('4 KiB frame allocator', 'Bitmap tracks the first 256 MiB'), ('4 MiB initial heap', 'Contiguous frames; free-list allocation'), ('Rust alloc collections', 'Vec, String and format!')]):
        y = 5.4+i*2.35
        d.band(p, 1.5, y, 12, label, d.CYAN, 1.7)
        d.text(p, 14.5, y+.28, 18, 1.1, body, 22, d.INK)
    d.callout(p, 'Addressability is not isolation: there are no per-app page tables or guard pages here.', d.AMBER)

    p = slide('Interrupts and time', 'Hardware events enter through owned gates.',
              'The kernel dispatches handlers and acknowledges the interrupt controller.',
              ['kernel/src/hal.rs', 'kernel/src/idt.rs', 'kernel/src/pit.rs', 'kernel/src/input.rs'],
              'The native PIC is remapped to 0x20/0x28. The PIT is configured for 100 Hz. IrqTable accepts Chain::None, installs a gate and unmasks the line. Dispatch checks spurious interrupts, calls the registered handler and sends EOI. Input IRQ handlers publish bytes to rings; decoding and shell work are deferred.')
    for i, label in enumerate(['Device IRQ', 'PIC + IDT', 'Rust handler', 'Input ring']):
        x = 1.5+i*7.9
        d.band(p, x, 5.7, 7.3, label, d.CYAN, 1.7)
        if i < 3:
            d.text(p, x+7.25, 6.0, .65, 1, '→', 20, d.MUTED, align='center')
    d.card(p, 1.5, 8.7, 15, 6.4, 'IRQ side', 'Bounded device work', 'Read bytes; update atomics; enqueue.\nNo allocation or BIOS call.', d.CYAN)
    d.card(p, 17.5, 8.7, 15, 6.4, 'Main loop', 'Decode and update Desk', 'Drain events; update shell state; draw.\nIdle paths can wait with hlt.', d.PURPLE)
    d.callout(p, 'Interrupt delivery is a mechanism; fair scheduling remains a separate system decision.', d.CYAN)

    p = slide('Drivers and platform contracts', 'One algorithm can serve two foundations.',
              'Shared drivers depend on small capabilities; the kernel owns their resources.',
              ['hal/Cargo.toml', 'kernel/src/hal.rs', 'hal/hal/src/irq.rs', 'hal/hal/src/mmio.rs'],
              'Examples of shared crates used by the kernel include serial, PIC, PIT, keyboard/i8042 and VGA. Raw port capability construction is unsafe at the platform edge. Driver algorithms remain safe Rust. Kernel IRQ registration changes shared tables with interrupts disabled. Current HAL status prose has historical wording; manifests and adapters show actual consumers.')
    d.card(p, 1.5, 5.6, 15, 9.3, 'Shared machinery', 'Device behaviour', 'Serial transmit and receive\nPIC and PIT register operations\nKeyboard decoding and PS/2 work\nVGA palette and display routines', d.GREEN)
    d.card(p, 17.5, 5.6, 15, 9.3, 'Native platform', 'Machine authority', 'Create the real port capability\nInstall and remove IRQ handlers\nExpose valid device memory\nManage critical sections', d.CYAN)
    d.callout(p, 'The platform supplies authority; drivers supply the reusable device logic.', d.GREEN)

    p = slide('The BIOS bridge', 'Protected mode can still reach firmware.',
              'A real-mode trampoline executes one BIOS call and restores kernel state.',
              ['kernel/src/bios.rs', 'kernel/bios16.asm', 'docs/N3-BIOS-PATH.md'],
              'NativeBios implements the shared Bios trait. The trampoline occupies 0x10000, with bounce buffer at 0x11000 and a dedicated real-mode stack. It saves/restores CPU state and PIC configuration, masks NMI during transition, accounts for BIOS ticks and drains input bytes left during the call. Kernel IRQ service is suspended even if firmware internally enables interrupts. The call is refused when paging is enabled or from a hardware interrupt handler; it is not re-entrant.')
    for y, label, accent in [(5.5, 'Shared VBE or BIOS client', d.GREEN), (7.7, 'NativeBios: validate context + take guard', d.CYAN), (9.9, 'Trampoline: real mode → INT n → protected mode', d.CYAN), (12.1, 'Restore CPU/PIC state + account for pending work', d.CYAN)]:
        d.band(p, 2.5, y, 29, label, accent, 1.6)
    d.text(p, 2.6, 14.35, 29, 1, 'Low-memory buffer bridges the addressing boundary.', 21, d.MUTED, align='center')
    d.callout(p, 'Useful firmware access today; calls can stall the kernel and are not preemptible.', d.AMBER)

    p = slide('From input to pixels', 'The native foundation already runs Desk.',
              'A recorded first-window frame from the native QEMU path.',
              ['kernel/src/shell.rs', 'docs/FIRST-WINDOW.md', 'docs/images/first-window/native-8-typed.png'],
              'The embedded image is an existing repository artefact, native-8-typed.png, from the first-window scenario. It is not a new screenshot from this presentation session. The report records equal DOS/native frame colours at both supported depths. First-window text fields and windows do not establish a separately loaded Notes app.')
    picture(p, ROOT/'docs/images/first-window/native-8-typed.png', 1.5, 5.4, 14.0, 10.5)
    d.text(p, 17, 5.65, 15.2, 1.4, 'A shared, deterministic path', 24, d.INK, True)
    d.text(p, 17, 7.6, 15.0, 5.9, 'PS/2 bytes become input events.\nThe shell updates window state.\nThe compositor tracks damage.\nA presenter writes device memory.\nThe DAC supplies the colours.', 22, d.MUTED)
    d.text(p, 17, 14.1, 15.0, 1.3, 'Recorded frames match the DOS track\nat 8-bit and 4-bit colour depths.', 19, d.GREEN, True)
    d.callout(p, 'The screenshot demonstrates shared shell behaviour, not a completed native app system.', d.PURPLE)

    p = slide('Safety and concurrency', 'Rust helps when the contracts stay true.',
              'Types support local reasoning; hardware lifetimes and CPU context still matter.',
              ['kernel/src/sync.rs', 'kernel/src/heap.rs', 'kernel/src/hal.rs', 'hal/hal/src/mmio.rs', 'docs/HAL.md'],
              'Current synchronization assumes one processor. without_interrupts preserves and restores the prior interrupt state; heap accesses use it. Shared shells and driver algorithms avoid unsafe code, while raw memory/CPU access lives in platform code or low-level HAL primitives. These rules do not imply resistance to arbitrary shared-address-space writes.')
    cards(p, [('Small unsafe edges', 'Establish validity', 'CPU state and port access\nMapped device regions\nInterrupt entry and lifetime'),
              ('One processor', 'Bound mutation', 'Interrupt-masked updates\nAtomic counters and rings\nKeep IRQ handlers short'),
              ('Future changes', 'Revisit assumptions', 'Paging and guarded stacks\nPreemptive execution\nProcesses or multiple CPUs')])
    d.callout(p, 'A safe method is only sound while its underlying mapping and ownership remain valid.', d.AMBER)

    p = slide('Faults and diagnostics', 'Make failure visible before adding recovery.',
              'Serial output and explicit exception reports keep early failures inspectable.',
              ['kernel/src/idt.rs', 'kernel/src/main.rs', 'kernel/src/exit.rs', 'tools/native-run.sh'],
              'Exception stubs report vector, registers, instruction pointer and control registers. Panic reports and exceptions end the test run. exit.rs writes to QEMU isa-debug-exit at 0xF4 and then halts; that device is a test harness mechanism, not physical hardware shutdown. This does not implement app-local fault containment or recovery.')
    d.card(p, 1.5, 5.6, 15, 9.2, 'Visible today', 'A diagnosable stop', 'Loader and kernel serial output\nException and register reports\nPanic diagnostics\nEmulator success/failure status', d.CYAN)
    d.card(p, 17.5, 5.6, 15, 9.2, 'Still a system policy', 'What happens afterwards?', 'Contain an app failure\nProtect kernel state\nRecover durable data\nRestart or shut down reliably', d.AMBER)
    d.callout(p, 'The emulator exit port is a test hook; real hardware needs an explicit shutdown policy.', d.AMBER)

    p = slide('Build and evidence', 'A reproducible image with several test views.',
              'Keep source checks, emulator behaviour and hardware claims distinct.',
              ['tools/native-build.sh', 'tools/native-image.sh', 'tools/native-test.sh', 'docs/NATIVE-EMULATORS.md', 'docs/NATIVE-BOOT.md'],
              'NASM assembles boot stages and the trampoline. Cargo/rust-lld builds the custom i486 ELF and elf2bin writes the flat image. native-image assembles the floppy image. QEMU -cpu 486 is the default automated target; Bochs and 86Box results cover selected additional paths as documented, not every feature in every emulator. Nothing has run on real hardware. No guest test was rerun while preparing this deck.')
    d.band(p, 1.5, 5.4, 31, 'NASM + Rust → kernel ELF → flat image → bootable disk image', d.CYAN, 1.6)
    cards_y = 8.1
    for i, (label, title, body, accent) in enumerate([
        ('Default gate', 'QEMU as a 486', 'Serial transcripts\nInjected input\nFrame comparisons', d.GREEN),
        ('Additional views', 'Bochs and 86Box', 'Selected boot/BIOS cases\nOlder firmware behaviour\nCPU-model differences', d.CYAN),
        ('Not established', 'Real hardware', 'No real-machine runs\nNo hardware timing claim\nNo audible-output claim', d.AMBER),
    ]):
        d.card(p, 1.5+i*10.65, cards_y, 9.7, 7.1, label, title, body, accent)
    d.callout(p, 'The image is testable; emulator agreement is not a hardware compatibility guarantee.', d.CYAN)

    p = slide('From kernel to operating system', 'The next layers must earn their contracts.',
              'The remaining work is about durable services and bounded app behaviour.',
              ['docs/DIRECTION.md', 'docs/OPEN-QUESTIONS.md', 'docs/RUST-DOS-INTEGRATION-ANALYSIS.md'],
              'Direction leaves scheduling, isolation and native app format open. BIOS disk probes exist, but storage/filesystem work remains. Bundled FreeDOS and V86 are later choices, not current compatibility services. Kernel and low-level runtime stay no_std; native std needs a real service implementation.')
    cards(p, [('Persist', 'Storage and files', 'Choose the initial disk path\nRead a filesystem reliably\nDefine writes and recovery'),
              ('Execute', 'Apps and services', 'Choose module format/ABI\nTrack ownership and unload\nExpose useful SDK services'),
              ('Protect', 'Failure boundaries', 'Choose scheduling policy\nBound stack and task work\nDecide process isolation')], d.AMBER)
    d.callout(p, 'FreeDOS integration and a native std layer are later consumers of these services.', d.PURPLE)

    p = slide('A reusable native foundation', 'Extract the kernel after proving composition.',
              'A clean split keeps desktop policy above machine services.',
              ['kernel/Cargo.toml', 'kernel/src/shell.rs', 'docs/REPOSITORY-BREAKUP-PROPOSAL.md'],
              'Today kernel/Cargo.toml directly depends on shell/UI/font crates, and main embeds a shared graphics spike scene. A proposed independent kernel needs shell-specific wiring and assets moved to a Desk-owned adapter. A final executable composes that adapter and kernel services. This is not implemented and does not require changing the app protection model.')
    for y, label, accent in [(5.5, 'desk-system: final executable + pinned release assembly', d.PURPLE), (10.7, 'Shared HAL contracts + reusable drivers', d.GREEN)]:
        d.band(p, 2, y, 30, label, accent, 1.8)
    d.band(p, 2, 8.1, 14.5, 'Desk native adapter', d.PURPLE, 1.8)
    d.band(p, 17.5, 8.1, 14.5, 'Native kernel services', d.CYAN, 1.8)
    d.text(p, 2.2, 13.5, 29.5, 1.6, 'Acceptance: kernel-only boot tests need no desktop assets;\nthe composed product still renders the same shared shell.', 22, d.MUTED, align='center')
    d.callout(p, 'Extract when the kernel has an independently testable consumer.', d.CYAN)

    p = slide('A suggested next sequence', 'Grow through small, observable milestones.',
              'A proposed path from the current kernel to a usable native system.',
              ['docs/DIRECTION.md', 'docs/RUST-DOS-INTEGRATION-ANALYSIS.md', 'docs/REPOSITORY-BREAKUP-PROPOSAL.md'],
              'This sequence is a recommendation, not a newly approved project roadmap. Keep the hosted track useful for shell/app iteration. Add targeted failure checks and performance budgets at each milestone. Stronger isolation and preemption should follow explicit product requirements, not repository count.')
    steps = [('1', 'Strengthen resource lifetimes', 'Mappings, handler removal and interrupt-masked work'),
             ('2', 'Read a real file into a native service', 'Storage path, filesystem parsing and bounded memory'),
             ('3', 'Load one app into the shared shell', 'Versioned SDK, events, ownership and unload'),
             ('4', 'Measure, harden and broaden machines', 'Latency, recovery, emulator diversity and hardware')]
    for i, (number, title, body) in enumerate(steps):
        y = 5.35+i*2.45
        d.band(p, 1.5, y, 2, number, d.CYAN, 1.8)
        d.text(p, 4.2, y, 28, .9, title, 23, d.INK, True)
        d.text(p, 4.2, y+1.02, 28, .85, body, 18, d.MUTED)
    d.callout(p, 'Keep each milestone bootable, inspectable and tested with its real consumer.', d.GREEN)

    p = slide('Source trail and scope', 'Inspect the implementation behind each claim.',
              'Per-slide source paths and additional context are in speaker notes.',
              ['docs/NATIVE-BOOT.md', 'docs/N3-BIOS-PATH.md', 'docs/HAL.md', 'docs/C-INPUT.md', 'docs/FIRST-WINDOW.md', 'docs/NATIVE-EMULATORS.md'],
              'The deck is grounded in the checkout on 24 October 2026. ODP generation, PDF rendering and slide review validate presentation artefacts only. Native builds, emulator runs and hardware checks were not repeated. Historical result scope is retained. All diagrams and text are editable; the native screenshot is an embedded repository PNG.')
    d.text(p, 1.6, 5.5, 15, .8, 'KERNEL AND MACHINE', 13, d.CYAN, True)
    d.text(p, 1.6, 6.8, 15, 6.8, 'kernel/src/main.rs\nkernel/src/memory.rs + heap.rs\nkernel/src/hal.rs + idt.rs\nkernel/src/bios.rs\ndocs/NATIVE-BOOT.md\ndocs/N3-BIOS-PATH.md', 20, d.MUTED)
    d.text(p, 18, 5.5, 14.5, .8, 'SHARING AND VERIFICATION', 13, d.PURPLE, True)
    d.text(p, 18, 6.8, 14.5, 6.8, 'docs/HAL.md\ndocs/C-INPUT.md\ndocs/FIRST-WINDOW.md\ndocs/NATIVE-EMULATORS.md\ntools/native-test.sh\ntools/shell-compare.sh', 20, d.MUTED)
    d.callout(p, 'A Rust kernel becomes useful as its service contracts become dependable.', d.CYAN)


if __name__ == '__main__':
    build()
    output = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT/'dist/presentations/rust-native-runtime.odp'
    d.write_deck(output, title='From the first instruction to Desk.',
                 subject='The native Rust foundation: boot, memory, interrupts, drivers and system services',
                 assets=ASSETS)
