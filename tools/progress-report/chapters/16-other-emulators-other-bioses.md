---
id: emulators
n: 16
title: Other emulators, other BIOSes
date: 2026-10-23
summary: Spike G booted the native image in Bochs and in 86Box, asked a 1993 BIOS for its memory and found that it had never heard of the calls the loader used, and caught QEMU being too kind to a 486.
prs: [16]
---
Every native boot in this archive so far was in QEMU, with QEMU's own BIOS and QEMU's idea of a 486. That is one opinion, repeated. Spike G (pull request #16, {{pr16.state}}, branch `spike/g-boot-bochs-86box`, built on `main`) asks the cheapest useful question: if the same floppy image meets a different emulator and a different BIOS, does it still boot? The answer is that it boots unchanged under Bochs 2.7 built as a 486, with three different BIOS images, and under 86Box 6.0 emulating a 33 MHz 486DX board with an AMIBIOS dated 8 August 1993, after one boot bug that only the second of those could find.

<!-- table g-results.md -->

**The bug the 1993 BIOS found.** On 86Box the first boot stopped at `stage2: no memory map from the BIOS`. The loader asked for the memory map with INT 15h function E820 and, failing that, with E801, and a BIOS from 1993 has neither: the spike's own note puts E801 in 1994. The fix is a third, older call, INT 15h AH=88h, which reports the kibibytes above one mebibyte in a single 16-bit count. The loader rewrites that count as the same kind of entries and tags the source as 3. The explainer is the chain as the loader's own source and test script define it.

<!-- explainer memchain -->

The same boot showed the A20 gate closed. No BIOS in QEMU or Bochs leaves it closed, so the loader's fast-gate code, which was only ever exercised by a test variant that closes the gate first, ran for real for the first time, and worked. With the fix the kernel runs to `boot complete` on 86Box and counts the 15,360 KiB the BIOS reports.

**QEMU is too kind to a 486.** `docs/NATIVE-BOOT.md` had said that `-cpu 486` makes QEMU reject instructions a 486 lacks. A one-sector probe run in both emulators shows that is only partly so.

<!-- table g-cpu-probe.md -->

QEMU removes `cmov` and `cmpxchg8b` but still runs `rdtsc`, `rdmsr`, MMX and reads of `CR4`; Bochs's 486 raises an invalid-opcode fault for all of them. The kernel passes everything on the Bochs 486, so it uses none of them, and the Bochs test keeps that claim honest. Bochs's 486 does answer `cpuid`, with a 486DX2 signature, which many real 486s would not, so the kernel must keep not using it either.

**Two quieter findings.** The state of `CR0` at entry depends on the BIOS: a 486 resets it with the cache-disable and not-write-through bits set, Bochs's legacy BIOS leaves them, and SeaBIOS and the AMIBIOS clear them. The kernel never touches the cache, so on a BIOS that leaves it off the kernel runs with the cache off, which works and is slow; the spike records enabling it as an open question and changes nothing blind. And Bochs honours the serial port's reset word length of five bits, so a program that writes to COM1 without setting 8N1 loses the top bits (an `S` arrives as `0x13`); QEMU ignores the word length. Stage 2 sets 8N1 first, which is why nothing broke.

**How it was done.** Everything runs in Docker and none of it is in CI, because the CI container has no Docker. Ubuntu's Bochs package cannot be a 486 (its models start at a generic part and a Pentium), so the spike builds Bochs 2.7 from the upstream tarball with the CPU level set to 4. Bochs has no exit device, so the runner polls the serial file and decides from the transcript. 86Box needed its COM1 attached to a pseudo-terminal, a message box dismissed with `xdotool`, and a 128-byte CMOS image made by driving the BIOS setup by hand once. The Bochs test takes about 20 seconds and 86Box's about 30, which is why only a normal boot and a divide error run in 86Box.

The spike's document mentions a screenshot of the 86Box run, kept as `dist/native-86box.png` by the runner. No image from the run is committed to the pull request, so there is none in this archive.

> **From the stacks — Counting memory in one 16-bit register.** Wikipedia lists INT 15h function 88h as "Get Extended Memory Size", describes E801h as a newer function from 1994, and says that what E820h returns supersedes both. As the story is usually told, the old call returns its answer as a count of kibibytes in one 16-bit register, which is why it could never see much, and the spike's document says it reports at most 15 MiB, or 63 MiB on some BIOSes. A loader from 1993 had no other choice; the Desk's loader keeps the old call as the last of three.
> Source: [BIOS interrupt call, Wikipedia](https://en.wikipedia.org/wiki/BIOS_interrupt_call)

```figures
id: memchain
kind: explainer
track: native
title: The loader's memory-map fallback chain
alt: A step-through of the three BIOS calls the loader tries for the memory map, E820 then E801 then AH=88, with the transcript line each one prints and the failure message if all three fail.
caption: E820, E801, AH=88: the order the loader tries and what 86Box's 1993 BIOS needed.
at: explainer memchain
thumb: steps
```

## The shelves

### Catalogued
- The native image boots to `boot complete` in Bochs 2.7 built as a 486 with `BIOS-bochs-legacy`, `BIOS-bochs-latest` and SeaBIOS 1.13, running the whole native test including the loader variants, the faults and the hang, in about 20 seconds; and in 86Box 6.0 on an `ami471` board with an AMIBIOS of 1993, running a normal boot and a divide error. The transcript is compared with `tests/native/expected.txt`, which accepts BIOS-dependent lines. CI is `{{pr16.ci}}` on the merge commit `{{pr16.head}}` of pull request #16, which is {{pr16.state}}, though CI itself does not run these emulator tests.
- The `e88` loader variant, built with `-DNO_E820 -DNO_E801`, boots under QEMU and Bochs and checks a rewritten transcript, so the AH=88 path is tested without 86Box.
- A probe shows which instructions QEMU's `-cpu 486` and Bochs's 486 reject; the Bochs test requires an invalid-opcode fault for `rdtsc`, `rdmsr`, `emms` and a read of `CR4`.

### Uncatalogued
- Real hardware, a real floppy drive or a real 486. A 1993 AMIBIOS in 86Box is the closest stand-in tried, and it emulates the ROM, the chipset and the floppy controller with the author's approximations.
- Cache and timing behaviour: no emulator here is cycle-accurate, and the kernel runs with whatever `CR0` state the BIOS leaves.
- Booting from a hard disk, a CD-ROM or USB, any machine but `ami471`, and the 86Box fault, hang and loader-variant matrix.
- Memory above 15 MiB on a BIOS that has only AH=88h.
- The emulator tests in CI: they run on a developer machine only.
- The 86Box picture of the boot, which was made and is not in the pull request.

### Wish list
- Enabling the cache on a 486 (`wbinvd`, then clearing the cache-disable bits), which depends on the chipset.
- A Bochs image prebuilt for CI, or a CI container with Docker, so the emulator tests are part of the checks.
- 86Box with more machines, disks and chipsets.
