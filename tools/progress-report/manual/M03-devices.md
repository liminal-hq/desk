---
id: m-devices
n: M3
part: I
title: The devices of a 486 PC
summary: The ISA bus and port I/O, the PICs, the PIT, the 8042 with the PS/2 keyboard and mouse, the 16550, VGA and VBE, the floppy and DMA as the BIOS sees them, and the BIOS services the project calls.
date: 2026-10-23
reviewed: 44f0265
watches: [hal/drivers/, hal/hal/src/port.rs, hal/hal/src/irq.rs, kernel/src/pic.rs, kernel/src/pit.rs, kernel/src/input.rs, kernel/src/gfx.rs, kernel/src/bios.rs, boot/stage2.asm]
log: [input, pixels, bios]
anatomy: [desk:emu, dos:vga, dos:lfb, dos:kbd]
---
## What it is

A 486-class PC is a processor and a handful of separate chips, each answering at a fixed address in a second, smaller address space and raising one of sixteen interrupt lines when it wants attention. This chapter introduces those chips, in the order the Desk meets them: the bus they sit on, the two chips that carry their interrupts to the processor, the timer, the keyboard controller with the keyboard and mouse behind it, the serial port, the video card, and the floppy drive as the BIOS presents it. It also names the BIOS services the project calls, and one service that is not the BIOS's at all: the mouse driver's INT 33h under DOS. The processor itself, with its real and protected modes, is taught in [M2](chapter:m-cpu); the way the project divides its code into HAL, drivers and platforms is on the map in [M1](chapter:m-map).

On the anatomy pages this chapter explains the boxes that touch the machine: [`emu`](chapter:anatomydesk#box-desk-emu), the emulators the native track runs on, and the hosted track's `vga`, `lfb` and `kbd`. It says what each device is, which ports and interrupt lines the code uses, and what the code does to it in outline. It does not repeat how the drivers are written. That is the job of [M10](chapter:m-sysdrivers) (serial, interrupt controllers, timer), [M11](chapter:m-input) (keyboard and mouse) and [M12](chapter:m-graphics) (VGA and VBE), which link back here for the hardware.

After this chapter you should be able to read a line such as `outb(0x43, 0x34)` and say which chip it addresses and what it asks, say why IRQ 12 reaches the processor as vector `0x2C`, decode a PS/2 mouse packet by hand, explain what mode 13h and the linear frame buffer are, and tell which of the project's device claims have been run in an emulator and which have not. Nothing here has run on a real PC. Wherever an emulator behaves more kindly than the hardware would, the chapter says so and names the document that records it.

## How it works

### The bus and port I/O

The devices sit on the **[ISA bus](term:isa-bus)**, the expansion bus of the IBM PC/AT, and the processor reaches them in two ways. Memory-mapped devices appear as ordinary addresses: the VGA text screen is memory at `0xB8000` and the graphics planes at `0xA0000`. Everything else appears in a separate **I/O address space** of 65,536 byte-wide ports, reached only by the instructions `in` and `out`. The HAL's `PortIo` trait wraps those instructions as `inb`, `outb`, `inw`, `outw`, `inl` and `outl` (`docs/HAL.md`, "Port I/O"). Every port in the table below is under `0x400`, which is where the ISA devices decode their addresses (the serial driver's comment says that ISA ports are below `0x400`).

**[Port I/O](term:port-io)** is not memory. Each access really happens, exactly once and in order, which is why the HAL implements it as `asm!` blocks the compiler may not remove or reorder (`docs/HAL.md`). Old chips are slow to settle between accesses, so the HAL has `io_wait`, a write to port `0x80`, the POST diagnostic port, which takes about a microsecond on the ISA bus by the document's figure and which the 8259 initialisation uses after each word. It is an elapsed-bus-cycle delay, not a time.

The table and drawing below are generated from the drivers' own constants. They list the ports this chapter relies on (the data port one above each 8259 command port, and COM3 and COM4, are named in the text only), each with the file that defines it, and the IRQ lines of the two interrupt controllers with the vectors the kernel and the BIOS give them. The build refuses a port whose constant has changed or an IRQ the HAL no longer names.

<!-- explainer m-ports -->

### The two 8259 interrupt controllers

A device raises an interrupt by pulling one of sixteen **[IRQ](term:irq)** lines. The processor has a single interrupt pin, so a **[PIC](term:pic)** (programmable interrupt controller, the Intel 8259) stands between them. The PC/AT has two: a master at ports `0x20` and `0x21` for IRQ 0 to 7 and a slave at `0xA0` and `0xA1` for IRQ 8 to 15, the slave's output wired to the master's IRQ 2. That is why the table shows IRQ 2 as the cascade, and why unmasking a slave line must also unmask IRQ 2 (`desk-pic` does both). Each controller has a command port and a data port one higher, which holds the mask: a set bit blocks that line.

When a line fires, the controller tells the processor a **vector number** (an index into the interrupt descriptor table, taught in [M2](chapter:m-cpu)), which is the controller's *base* plus the line's number within it. The base is the controller's to choose, and the PC's default is a trap. The BIOS leaves the master at base `0x08` and the slave at `0x70` (`BIOS_BASES` in `hal/drivers/pic/src/lib.rs`), so the timer is vector `0x08`. The processor already uses vectors 0 to 31 for its own exceptions, so IRQ 0 would arrive looking like a double fault. The kernel therefore reprograms both chips to `0x20` and `0x28` (`REMAPPED_BASES`), and the transcript says so: `pic: remapped to 0x20 and 0x28, only IRQ0 unmasked`. M6 follows the kernel's side.

Reprogramming is a fixed sequence of **initialisation words**. The driver's test, `native_init_matches_the_kernel_with_its_delays`, spells it out: `0x11` to each command port (edge-triggered, cascaded, a fourth word follows); the base to each data port (`0x20`, then `0x28`); the wiring (`0x04` to the master, meaning the slave is on line 2, and `0x02` to the slave, its identity); `0x01` to each for 8086 mode; and finally the masks, `0xFE` and `0xFF`, which leave only the timer open. An `io_wait` follows every word.

At the end of an interrupt the handler must send an **[end of interrupt](term:eoi)** (EOI), the command byte `0x20` to a command port, or the controller will not deliver that line again. For a slave line the EOI goes to the slave and then to the master, as the test `a_slave_line_is_acknowledged_on_both_controllers` shows: the keyboard needs one write, `(0x20, 0x20)`, and the mouse needs two, `(0xA0, 0x20)` then `(0x20, 0x20)`. Lines 7 and 15 can also fire spuriously, when a request vanishes before the processor asks who sent it. The driver tells the real from the spurious by reading the in-service register: it writes `0x0B` to the command port and reads it back, and a spurious IRQ 15 gets an EOI on the master only.

### The 8254 timer

The **[PIT](term:pit)** (programmable interval timer, the Intel 8254) divides an input clock of 1,193,182 Hz (`INPUT_HZ`) by a number you give it and raises IRQ 0 at the result. Its channel 0 counter is at port `0x40` and its command register at `0x43`. To program it you write a command byte to `0x43` that says which channel, how the 16-bit divisor will be sent (low byte then high byte) and which mode, then write the two divisor bytes to `0x40`. The kernel and `desk-pit` use mode 2, a rate generator, with the command `0x34`. The BIOS runs the same channel in mode 3 (a square wave, command `0x36`) with a divisor of 65,536, written as zero, which is about 18.2 Hz. The driver has `restore_bios_rate`, which programs exactly that, and its test checks the mode, the zero and the 18,207 millihertz it works out to. Nothing in the repository calls it yet.

### The 8042 and the PS/2 devices

On the PC/AT the keyboard connects through the **[8042](term:8042)**, a small microcontroller on the motherboard with two ports: data at `0x60`, and at `0x64` a status register (when read) and a command register (when written). The PS/2 generation added a second channel for a mouse behind the same chip. The status register's bit 0 says a byte is waiting at `0x60`, bit 1 says the controller has not yet taken the last byte you sent (so you must wait before writing), and bit 5 says the waiting byte came from the mouse rather than the keyboard. The controller itself has commands: `0xAA` for a self-test (the answer `0x55` means it passed), `0xAD` and `0xAE` to disable and enable the keyboard, `0xA7` and `0xA8` for the mouse, `0x20` and `0x60` to read and write its *configuration byte*, and `0xD4` to send the next data byte to the mouse instead of the keyboard. The configuration byte holds the enable bits for IRQ 1 (the keyboard, bit 0) and IRQ 12 (the mouse, bit 1) and bit 6, which asks the controller to translate the keyboard's scancodes. All of these are named constants in `hal/drivers/i8042/src/lib.rs`.

The keyboard sends **[scancodes](term:scancode)**: one byte when a key goes down, another when it comes up (the release has bit 7 set), with `0xE0` as a prefix for the keys added on the 101-key layout. Keyboards speak scancode set 2 after a reset, and the 8042 can translate that to the older set 1 on the way through; the Desk reads set 1 (`docs/C-INPUT.md`, "The set-up sequences"). The transcript line `input: keyboard ready, scancode set 1 translated by the controller` is that translation turned on.

The mouse is a different protocol on the same wire. After the reset-and-enable handshake it streams packets of three bytes. The first holds the three buttons in bits 0 to 2, a bit that is always set (bit 3, which lets a receiver recognise a first byte), the sign bits of the two movements in bits 4 and 5, and overflow flags in bits 6 and 7. The second byte is the low eight bits of the horizontal movement and the third of the vertical, so each movement is a nine-bit two's complement number with its sign in the first byte. A mouse with a wheel, found by a probe that sets the sample rate to 200, 100 and 80 and reads its ID (3), sends a fourth byte of wheel movement. The device reports *up* as positive; the driver negates it, so that down the screen is positive like everything else the Desk draws. The worked example below decodes real packets from the driver's test.

### The 16550 serial port

The **[UART](term:uart)** (universal asynchronous receiver-transmitter) is the chip behind a serial port; the PC's is the National Semiconductor 16550 or compatible, with a 16-byte FIFO. COM1 is at `0x3F8` and COM2 at `0x2F8` (the driver also names `0x3E8` and `0x2E8`), and each chip has eight registers at consecutive ports from its base: data, interrupt enable, FIFO control, line control, modem control and line status among them. The same port numbers mean different registers when the *divisor latch* bit (DLAB, bit 7 of line control) is set: the first two then hold the divisor. The baud rate is 115,200 divided by that divisor. `Serial16550::init` writes seven values in a fixed order (interrupts off, DLAB on, divisor low, divisor high, 8 data bits no parity one stop bit, FIFOs on and cleared, DTR and RTS and OUT2 set), and the host test `init_matches_the_kernel_byte_for_byte` checks them against the kernel's original sequence: for 115,200 baud the divisor is 1. Writing a byte waits, boundedly, for bit 5 of the line status register (transmitter holding register empty). The project uses the serial port as its console: every line of the boot transcript is written to COM1, which QEMU captures to a file.

### VGA

The **[VGA](term:vga)** (video graphics array) is IBM's 1987 display adapter, still the baseline that every PC video card imitates. It is a bank of registers behind ports `0x3C0` to `0x3DA` and 256 KiB of memory in four 64 KiB *planes* that appear at `0xA0000`. The registers come in indexed groups: you write the register number to an address port and the value to the data port next to it. The groups are the sequencer (`0x3C4`, `0x3C5`), the CRT controller (`0x3D4`, `0x3D5`), the graphics controller (`0x3CE`, `0x3CF`), the attribute controller (`0x3C0`, a single port that alternates between index and data), and the miscellaneous output register (`0x3C2`). A **mode** is a set of values for all of them. `desk-vga` holds the complete tables for three:

- **Text mode 03h.** 80 columns by 25 rows of character cells, 720 by 400 pixels on the monitor, in memory at `0xB8000` as 16-bit cells of a character byte and an attribute byte. The kernel's console (`kernel/src/vga.rs`) writes this memory directly. The glyph shapes come from a font held in one of the planes, which is why the loader copies the BIOS's 8 by 16 font to `0x2000` before the kernel leaves text mode ([M4](chapter:m-loader)).
- **[Mode 13h](term:mode-13h).** 320 by 200 pixels with 256 colours, one byte per pixel, in a single linear block at `0xA0000`. The code's comment in `regs.rs` describes it as chain-4 (the hardware spreads consecutive bytes across the four planes for you), 70 Hz, with 400 lines doubled to 200.
- **[Planar](term:planar-mode) mode 12h.** 640 by 480 pixels with 16 colours at 60 Hz. A pixel's four bits are in four different planes at the same address, so drawing has to select planes through the sequencer's map mask and the graphics controller's set/reset and bit-mask registers. A byte at `0xA0000 + y × 80 + x ÷ 8` holds eight pixels' worth of one plane.

The colours are not in the frame buffer. A pixel value is an index into the **[DAC](term:dac)** (digital-to-analogue converter), a 256-entry table of colours inside the card. You write an index to port `0x3C8` (to write) or `0x3C7` (to read), then red, green and blue in turn to `0x3C9`, six bits each, and the index advances by itself after each triple, so a whole palette is one index write and 768 data writes. Port `0x3C6` is a mask that must be `0xFF` for every entry to be reachable. This is why the Desk's palette fades are a matter of rewriting the table, not the screen. Bit 3 of the input status register at `0x3DA` is high during the vertical retrace, the gap between frames, and the driver waits for it (with a bound) before each palette write so that the change does not tear.

### VBE and the linear frame buffer

VGA stops at 640 by 480. The video card makers added their own higher modes, each programmed differently, and the **[VBE](term:vbe)** (VESA BIOS Extensions) standard gave them one BIOS interface: INT 10h with AH=4Fh. The functions the project uses are 4F00h (controller information), 4F01h (information about one mode), 4F02h (set a mode), 4F03h (read the current mode) and 4F05h (move the bank window, wrapped and tested but not used). Setting a mode with bit 14 (`0x4000`, `MODE_LINEAR`) ORed into the mode number asks for the **[linear frame buffer](term:lfb)** (LFB): the whole screen as one flat block of memory at a physical address the mode information gives, instead of 64 KiB seen through a movable window at `0xA0000`. VBE 2.0 introduced it, and the loader refuses a controller older than 2.0 (`docs/D-NATIVE-VGA.md`, "The loader: VBE before protected mode"). On QEMU's standard card the loader's search finds mode `0x0101`, 640 by 480 at 8 bits per pixel, with the frame buffer at `0xFD000000`.

> **From the stacks — The mode with a flat screen.** Wikipedia's article on mode 13h calls it the standard 256-colour mode on MCGA and VGA hardware, introduced in 1987 with the IBM PS/2, with a resolution of 320 by 200 pixels. It says the screen buffer is simply the bytes in segment `0xA000` from offset 0 to 63,999, and that each palette entry has 18 bits of colour, 6 per RGB channel. The article carries maintenance notices about missing citations, so its claims are not fully verified.
> Source: [Mode 13h, Wikipedia](https://en.wikipedia.org/wiki/Mode_13h)

> **From the stacks — Why the buffer had to be linear.** Wikipedia's article on the VESA BIOS Extensions lists VBE 2.0 as November 1994, and says it added linear framebuffer access, which enables direct access in protected mode as one large area of memory instead of smaller chunks. It also says that VBE 1.2 was defined in 1991 and that these early versions require real mode to work. The article says that before protected-mode banking, setting a graphics mode meant doing it before entering protected mode or using V86 mode.
> Source: [VESA BIOS Extensions, Wikipedia](https://en.wikipedia.org/wiki/VESA_BIOS_Extensions)

### The floppy, DMA and the BIOS services

The project has no driver for the **floppy controller**. It never touches the controller's ports. Every floppy read, in the loader and in the kernel's test, goes through the BIOS: INT 13h, the disk service. What it still depends on is how the hardware behaves behind that call. The controller raises IRQ 6 when an operation finishes, which is why the kernel leaves IRQ 6 open during a BIOS call (the mask table in `docs/N3-BIOS-PATH.md`). It moves data into memory by **[DMA](term:dma)**, using the 8237 DMA chip of the PC, so the processor does not copy the sector. The DMA chip works on 64 KiB pages and below 16 MiB, and a transfer cannot cross a page boundary; `boot/stage1.asm` splits its reads for that reason ([M4](chapter:m-loader)), and `docs/N3-BIOS-PATH.md` places the kernel's bounce buffer inside one 64 KiB block for the same reason. No code of the project programs the DMA chip, so its ports are not listed in the table above.

The BIOS is firmware that answers software interrupts, and it exists only in real mode. The services the project calls, and who calls them, are these (each row is a call in the code named):

| Interrupt and function | What it does | Called by |
|---|---|---|
| INT 10h AH=0Eh | Teletype output to the screen | `putc` in `boot/stage1.asm` and in `boot/stage2.asm` |
| INT 10h AX=1130h BH=06h | Pointer to the 8 by 16 font | `load_font` in `boot/stage2.asm` |
| INT 10h AX=4F00h, 4F01h, 4F02h | VBE controller information, mode information, set mode | `video_setup` in `boot/stage2.asm` |
| INT 10h AX=0003h | Set text mode | `leave_vbe` in `kernel/src/gfx.rs`, through the trampoline |
| INT 12h | Conventional memory size in KiB | `map_conventional` in `boot/stage2.asm`; the kernel's BIOS test |
| INT 13h AH=00h, 02h | Reset the disk system; read sectors by CHS | `read_chunk` in `boot/stage1.asm`; the kernel's BIOS test |
| INT 13h AH=42h | Extended read by [LBA](term:lba), for hard disks | The kernel's BIOS test, on an IDE disk |
| INT 15h AX=E820h | The memory map, one entry per call | `map_e820` in `boot/stage2.asm`; the kernel's BIOS test |
| INT 15h AX=E801h and AH=88h | Older memory sizes | `map_e801` and `map_88`; the kernel's BIOS test |
| INT 15h AH=86h | Wait a number of microseconds | The kernel's BIOS test |
| INT 1Ah AH=00h, 02h, 04h | Ticks since midnight; real-time clock time and date | The kernel's BIOS test |

What the [E820](term:e820) call returns, and how the older [E801](term:e801) and AH=88h calls are rewritten into the same shape, is in M4. The loader calls the BIOS in real mode, before the switch. The kernel can call it afterwards only through a trampoline that drops back to real mode for the length of one call, and that, and what it must do to the PICs while it is there, is [M8](chapter:m-bioscalls).

### On DOS, the mouse is not a BIOS device

Under DOS the Desk does not talk to the 8042's mouse channel. A DOS program asks the mouse *driver*, a program loaded from `CONFIG.SYS` or `AUTOEXEC.BAT`, which hooks software interrupt 33h. The comment at the top of `hal/drivers/int33/src/lib.rs` names the examples: CTMOUSE, the vendor's own driver, or the one built into DOSBox. The interface is not part of the BIOS and a machine may have no driver at all, in which case the hosted platform runs keyboard-only. The crate uses function 0000h to reset and detect the driver (AX comes back as FFFFh when one is installed), 0003h for the button state and 000Bh for the motion since the last call, in *mickeys* (the mouse's own counting units, a unit of movement that the driver does not convert to pixels), which `MouseInput` turns into the same `MouseEvent` the PS/2 packet decoder produces, so that the shared core cannot tell the two tracks apart. The keyboard on DOS is the opposite case: the hosted platform does take over IRQ 1, with a protected-mode handler, and reads port `0x60` itself.

## Why it is this way

- **Reprogram the PICs, do not live with the default.** The BIOS vectors overlap the processor's exceptions, so a native kernel must move them, and the HAL's `desk-pic` serves both directions, so the sequence exists once: the same `Pic8259::init` writes the kernel's bases and, for a BIOS call, puts the BIOS's back (`docs/N3-BIOS-PATH.md`, "Interrupts, the PICs, the timer and the keyboard"). Under DOS the runtime does not remap them: `spikes/d0-rust-dos/rt/src/irq.rs` asks the DPMI host for the vector of each line and hooks that.
- **The timer at 100 Hz, the divisor rounded to nearest.** The kernel's rate is one constant, `HZ` in `kernel/src/pit.rs`, and `divisor_for` rounds to the closest divisor, as the kernel did before the driver was shared; a test pins 100 Hz to divisor 11,932. Because the PIT stays at 100 Hz and the BIOS's interrupt handler counts the same interrupts, a BIOS tick counted during a call is exactly one kernel tick (`docs/N3-BIOS-PATH.md`).
- **The 8042 driver never touches the output port.** Bit 1 of the controller's output port is also the A20 gate, and bit 0 must stay set or the machine resets. The driver therefore avoids the output-port commands `0xD0` and `0xD1` and the reset pulse, and a host test asserts it (`docs/C-INPUT.md`, "The 8042 controller"). The loader's keyboard-controller A20 method uses that port; the kernel does not.
- **Every wait is bounded.** A floating bus reads all ones, so a status bit may never change. Waits on the 8042 are counted reads (`POLLS` is 100,000 and `SLOW_POLLS` 1,000,000) and the retrace wait is a million reads per phase (`VRETRACE_POLLS`); on running out, the call returns an error rather than hanging. The document's conversion of those counts to milliseconds assumes about a microsecond per ISA read and is, in its own words, an estimate.
- **The handlers move bytes and decode nothing.** The IRQ 1 and IRQ 12 handlers read a byte from `0x60` and put it in a lock-free ring; the mouse handler assembles a packet because it cannot wait. They check bit 5 of the status register so each takes only its own device's byte (`docs/C-INPUT.md`, "The interrupt hand-off"; M11).
- **Register-programmed modes as well as VBE.** The native kernel can set mode 13h or 12h by writing the tables, with no BIOS at all, and can ask VBE only before the switch or through the trampoline. Leaving VBE without the BIOS is card-specific: on QEMU's standard card it takes one write to the Bochs VBE registers (`0x1CE` and `0x1CF`), and on QEMU's Cirrus card the same programming left a garbled screen, which is why the kernel now asks the BIOS for text mode 3 on any card that has no Bochs registers (`docs/D-NATIVE-VGA.md`, "QEMU against real cards"). That is the clearest case in the project of an emulator model hiding a hardware difference.
- **The DAC owned by the Desk.** The palette is written by the Desk, never through the frame buffer (`docs/PALETTE.md`), because only the DAC can fade the whole screen in 16 writes of 768 values.

## The code

| Where | What it holds |
|---|---|
| [`hal/hal/src/port.rs`](repo:hal/hal/src/port.rs) | [`PortIo`](sym:hal/hal/src/port.rs::PortIo), the trait through which every driver reaches the I/O space, and [`POST_PORT`](sym:hal/hal/src/port.rs::POST_PORT) for the `io_wait` delay |
| [`hal/hal/src/irq.rs`](repo:hal/hal/src/irq.rs) | [`Irq`](sym:hal/hal/src/irq.rs::Irq) and the named lines, `TIMER` to `IDE_SECONDARY_OR_SPURIOUS` |
| [`hal/drivers/pic/src/lib.rs`](repo:hal/drivers/pic/src/lib.rs) | [`Pic8259`](sym:hal/drivers/pic/src/lib.rs::Pic8259): initialisation words, masks, EOI, the in-service and request registers; [`REMAPPED_BASES`](sym:hal/drivers/pic/src/lib.rs::REMAPPED_BASES) and [`BIOS_BASES`](sym:hal/drivers/pic/src/lib.rs::BIOS_BASES) |
| [`hal/drivers/pit/src/lib.rs`](repo:hal/drivers/pit/src/lib.rs) | [`divisor_for`](sym:hal/drivers/pit/src/lib.rs::divisor_for), [`Pit`](sym:hal/drivers/pit/src/lib.rs::Pit) (programs channel 0, reads the counter, restores the BIOS rate) and [`PitClock`](sym:hal/drivers/pit/src/lib.rs::PitClock) |
| [`hal/drivers/serial/src/lib.rs`](repo:hal/drivers/serial/src/lib.rs) | [`Serial16550`](sym:hal/drivers/serial/src/lib.rs::Serial16550): `init`, `probe`, `write_byte`, [`COM1`](sym:hal/drivers/serial/src/lib.rs::COM1) |
| [`hal/drivers/i8042/src/lib.rs`](repo:hal/drivers/i8042/src/lib.rs) | The ports, the [`command`](sym:hal/drivers/i8042/src/lib.rs::command), [`status`](sym:hal/drivers/i8042/src/lib.rs::status) and [`config`](sym:hal/drivers/i8042/src/lib.rs::config) constants, controller, keyboard and mouse set-up |
| [`hal/drivers/i8042/src/irq.rs`](repo:hal/drivers/i8042/src/irq.rs) | [`KbdIrq`](sym:hal/drivers/i8042/src/irq.rs::KbdIrq), [`MouseIrq`](sym:hal/drivers/i8042/src/irq.rs::MouseIrq) and [`decode`](sym:hal/drivers/i8042/src/irq.rs::decode), the packet decoder |
| [`hal/drivers/kbd/src/lib.rs`](repo:hal/drivers/kbd/src/lib.rs) | [`Decoder`](sym:hal/drivers/kbd/src/lib.rs::Decoder): scancode set 1 to key events |
| [`hal/drivers/int33/src/lib.rs`](repo:hal/drivers/int33/src/lib.rs) | The DOS mouse driver interface: [`VECTOR`](sym:hal/drivers/int33/src/lib.rs::VECTOR), the [`func`](sym:hal/drivers/int33/src/lib.rs::func) numbers, [`Mouse`](sym:hal/drivers/int33/src/lib.rs::Mouse) |
| [`hal/drivers/vga/src/regs.rs`](repo:hal/drivers/vga/src/regs.rs) | The register ports and the tables [`MODE_13H`](sym:hal/drivers/vga/src/regs.rs::MODE_13H), [`MODE_12H`](sym:hal/drivers/vga/src/regs.rs::MODE_12H) and [`MODE_03H`](sym:hal/drivers/vga/src/regs.rs::MODE_03H) |
| [`hal/drivers/vga/src/dac.rs`](repo:hal/drivers/vga/src/dac.rs) | The palette ports, [`set_palette`](sym:hal/drivers/vga/src/dac.rs::set_palette), [`wait_vretrace`](sym:hal/drivers/vga/src/dac.rs::wait_vretrace) and fades |
| [`hal/drivers/vga/src/vbe.rs`](repo:hal/drivers/vga/src/vbe.rs) | [`MODE_LINEAR`](sym:hal/drivers/vga/src/vbe.rs::MODE_LINEAR) and the calls 4F00h to 4F05h over the `Bios` trait |
| [`kernel/src/vga.rs`](repo:kernel/src/vga.rs) | The kernel's text console, written straight to `0xB8000` |
| [`kernel/src/pic.rs`](repo:kernel/src/pic.rs), [`kernel/src/pit.rs`](repo:kernel/src/pit.rs), [`kernel/src/input.rs`](repo:kernel/src/input.rs) | How the kernel programs the PICs and the PIT and brings up the 8042 devices |
| [`boot/stage2.asm`](repo:boot/stage2.asm) | The loader's real-mode BIOS calls: the memory map, the font, VBE |
| [`tools/progress-report/manual/explainers/ports.py`](repo:tools/progress-report/manual/explainers/ports.py) | The explainer above, which reads the constants |

For depth: `docs/HAL.md` ([port I/O](doc:docs/HAL.md#port-io-built), [input](doc:docs/HAL.md#input-built-by-spike-c-for-the-kernel-and-spike-b-for-dos)), `docs/C-INPUT.md` ([the 8042 controller](doc:docs/C-INPUT.md#the-8042-controller), [the mouse packet](doc:docs/C-INPUT.md#the-mouse-packet)), `docs/B-DOS-INPUT.md`, `docs/A-VGA-VBE.md`, `docs/D-NATIVE-VGA.md` and `docs/N3-BIOS-PATH.md` ([interrupts, the PICs, the timer and the keyboard](doc:docs/N3-BIOS-PATH.md#interrupts-the-pics-the-timer-and-the-keyboard)).

## Worked example

**The PIT at 100 Hz.** The kernel asks for 100 Hz. `divisor_for` computes `(1,193,182 + 100 ÷ 2) ÷ 100` in whole numbers, which is 1,193,232 ÷ 100 = 11,932 (the fraction .32 is dropped; adding half the rate first made it round to nearest). In hexadecimal 11,932 is `0x2E9C`. The driver writes three bytes, the sequence the test `programming_100_hz_writes_what_the_kernel_writes` scripts: `0x34` to port `0x43`; then `0x9C` to port `0x40`; then `0x2E` to port `0x40`. The command byte `0x34` is `00 11 010 0` in binary: channel 0, low byte then high byte, mode 2, binary counting. The chip really delivers 1,193,182 ÷ 11,932 = 99.998 Hz, which `actual_millihertz` reports as 99,998 (the driver's own comment says so), so a "tick" is 10.0002 ms and 100 of them are a hair longer than a second. The transcript line `timer: waited for 10 ticks with hlt, elapsed 10 ticks` is that timer working in QEMU.

**A mouse packet.** The driver's test `packets_decode_with_sign_extension_and_a_flipped_y` feeds `decode` the packet `38 FF FE`. The first byte is `0x38`, binary `0011 1000`: buttons none (bits 0 to 2 clear), bit 3 set as it must be, the X sign bit (bit 4) set and the Y sign bit (bit 5) set, no overflow. The X byte `0xFF` is 255, and with the sign bit it is 255 − 256 = −1: one step left. The Y byte `0xFE` is 254, and with its sign bit 254 − 256 = −2, which the device means as two steps *down* (it reports up as positive), and the driver negates it, so the event is `dx = -1, dy = +2`: left one, down two on the Desk's screen. A second packet, `09 0A 04`, from the test `mouse_packets_become_events_and_the_acknowledgement_covers_the_slave`, is `0x09` (the left button, bit 3) with dx 10 and device dy 4, which becomes dx +10, dy −4 and `buttons = 1`. The nine-bit extremes the test also checks are −256 (sign bit set and the byte `0x00`) and +255. A byte `0x48` or `0x88` as the first byte has an overflow bit set; the packet is consumed and produces no event.

**IRQ 12 to vector 0x2C.** The mouse is IRQ 12, on the slave, line 4 of it (12 − 8). The kernel's slave base is `0x28`, so the vector is `0x28 + 4 = 0x2C`. Under the BIOS's base of `0x70` the same line is `0x74`. The table above has the row. To open it the kernel's registry unmasks IRQ 12 on the slave *and* the cascade, IRQ 2, on the master; the PIC test shows the pair of writes (`0xEF` to port `0xA1`, then `0xFB` to port `0x21`).

## State of this layer

Verified here means that the record cites a test that runs on the host (`cargo test` in `hal/`, against a model of the chip on a mock port bus) or in the native boot test on QEMU. It never means a real chip. Where a host model and the real chip could differ, the note says so.

```status
id: devices.portio
label: Port I/O through PortIo, with the POST-port delay
state: built-verified
evidence: sym:hal/hal/src/port.rs::POST_PORT, test:hal/drivers/pic/src/lib.rs::native_init_matches_the_kernel_with_its_delays
note: The kernel and the loader use the real in and out instructions in QEMU; the host tests use a mock bus that records the accesses.

id: devices.pic
label: Two 8259s remapped to 0x20 and 0x28, masked, acknowledged and told apart from spurious lines
state: built-verified
evidence: test:tests/native/expected.txt::pic: remapped to 0x20 and 0x28, test:hal/drivers/pic/src/lib.rs::spurious_interrupts_are_told_apart_by_the_in_service_register, sym:hal/drivers/pic/src/lib.rs::REMAPPED_BASES
note: QEMU's controllers are an emulation. The document that records the reinitialisation says a chipset with an integrated controller is the thing most likely to differ.

id: devices.pit
label: The 8254 at 100 Hz with divisor 11,932 (restoring the BIOS rate is written and host-tested, but nothing calls it yet)
state: built-verified
evidence: test:hal/drivers/pit/src/lib.rs::programming_100_hz_writes_what_the_kernel_writes, test:tests/native/expected.txt::timer: waited for 10 ticks with hlt, sym:hal/drivers/pit/src/lib.rs::INPUT_HZ

id: devices.uart
label: The 16550 at 115,200 baud as the console, written to COM1
state: built-verified
evidence: test:hal/drivers/serial/src/lib.rs::init_matches_the_kernel_byte_for_byte, test:tests/native/expected.txt::boot complete, sym:hal/drivers/serial/src/lib.rs::COM1
note: Every native test reads the serial port of QEMU. COM1 on a real machine may hold a mouse or a terminal; docs/HAL.md says so.

id: devices.i8042
label: The 8042 set-up, with the A20 output-port commands never used
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::controller_init_runs_the_documented_sequence_and_reports_what_it_found, test:hal/drivers/i8042/src/tests.rs::controller_init_never_touches_the_output_port_where_the_a20_gate_lives, test:tests/native/expected.txt::input: 8042 dual channel
note: QEMU answers instantly. docs/C-INPUT.md lists what real controllers, USB legacy emulation and laptop pads may do differently.

id: devices.keyboard
label: The PS/2 keyboard in scancode set 1 through IRQ 1
state: built-verified
evidence: test:tests/native/expected.txt::input: keyboard ready, test:tests/native/expected-input.txt::input: IRQ 1 and IRQ 12 handlers registered, sym:hal/drivers/kbd/src/lib.rs::Decoder

id: devices.mouse
label: The PS/2 mouse: ID 3, four-byte packets and the packet decoder
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::packets_decode_with_sign_extension_and_a_flipped_y, test:tests/native/expected.txt::input: mouse ready, sym:hal/drivers/i8042/src/irq.rs::decode
note: QEMU never loses a byte, so the resynchronisation after a lost byte runs only in host tests.

id: devices.vga
label: Mode 13h, mode 12h and text mode 03h by register programming, and the DAC
state: built-verified
evidence: test:hal/drivers/vga/tests/registers.rs::mode_13h_writes_the_documented_sequence, test:hal/drivers/vga/tests/dac.rs::a_palette_is_one_index_write_and_triples_of_data, test:tests/native/expected-gfx-fallback.txt::gfx: mode 13h by registers
note: The tables were checked against DOSBox's BIOS and QEMU's card. A monitor was never attached, so the timing values are unseen (docs/A-VGA-VBE.md).

id: devices.vbe
label: VBE 2.0 with a linear frame buffer, or a recorded reason why not
state: built-verified
evidence: test:tests/native/expected-gfx-vbe.txt::stage2: vbe: mode set with the linear frame buffer, test:hal/drivers/vga/tests/vbe.rs::set_mode_adds_the_linear_bit, sym:hal/drivers/vga/src/vbe.rs::MODE_LINEAR
note: Verified on QEMU's standard and Cirrus cards and the other emulators in docs/N3-BIOS-PATH.md. Banked modes are wrapped but unused, and real cards may list other modes.

id: devices.bios
label: The BIOS services the kernel calls: INT 12h, 13h, 15h and 1Ah
state: built-verified
evidence: test:tests/native/expected-bios.txt::bios: INT 12h, test:tests/native/expected-bios.txt::bios: INT 1Ah agrees with the RTC read directly through the CMOS ports to within 2 s, sym:kernel/src/bios.rs::call
note: SeaBIOS and Bochs's BIOSes, never a real 486 BIOS (docs/N3-BIOS-PATH.md).

id: devices.floppy
label: The floppy read through INT 13h and the 64 KiB DMA rule
state: built-verified
evidence: test:tests/native/expected.txt::boot complete, sym:boot/stage1.asm::read_chunk
note: QEMU may not enforce the 64 KiB rule itself, and no code touches the floppy controller or the DMA chip. The retry path has never run (see M4).

id: devices.fdc-driver
label: A native floppy or DMA driver
state: planned
evidence: doc:docs/HAL.md#storage-sketch-the-native-storage-milestone-builds-it

id: devices.int33
label: The DOS mouse through INT 33h, and the keyboard through a protected-mode IRQ 1 handler
state: built-unverified
evidence: sym:hal/drivers/int33/src/lib.rs::VECTOR, test:hal/drivers/int33/src/tests.rs::motion_buttons_and_the_wheel_become_mouse_events, doc:docs/B-DOS-INPUT.md#what-was-and-was-not-verified
note: Tested against a model of the driver, and by hand in DOSBox. The first-window run scripts the pointer as events and never moves a real mouse, so the anatomy page tags the box unverified too. The keyboard path is run by the first-window check.

id: devices.hardware
label: Any of the above on a real PC
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#what-was-and-was-not-verified
```

The interrupt controllers, the timer, the 8042 and the serial port are {{state.devices.pic}} in QEMU and on the host. VGA and VBE are {{state.devices.vbe}} on QEMU's cards and in DOSBox, and the DOS mouse is {{state.devices.int33}}. A real machine is {{state.devices.hardware}}.

## Try it yourself

These commands run the host tests of the drivers this chapter describes and read the constants the explainer reads. They need a Rust toolchain on the host (`cargo`) and `grep`. QEMU is not needed.

```try
needs: host: cargo, grep
ran: 2026-10-23 on the host at 44f0265
$ cd hal && cargo test -p desk-pit programming_100_hz 2>&1 | grep -E "^test "
test tests::programming_100_hz_writes_what_the_kernel_writes ... ok
$ cd hal && cargo test -p desk-i8042 packets_decode 2>&1 | grep -E "^test "
test tests::packets_decode_with_sign_extension_and_a_flipped_y ... ok
$ cd hal && cargo test -p desk-pic 2>&1 | grep -E "^test "
test tests::a_slave_line_is_acknowledged_on_both_controllers ... ok
test tests::a_spurious_irq_15_is_acknowledged_on_the_master_only ... ok
test tests::masking_reads_modifies_and_writes_one_bit ... ok
test tests::spurious_interrupts_are_told_apart_by_the_in_service_register ... ok
test tests::the_service_and_request_registers_are_read_through_ocw3 ... ok
test tests::native_init_matches_the_kernel_with_its_delays ... ok
$ cd hal && cargo test --workspace 2>&1 | grep -c "test result: ok"
23
$ grep -n "pub const CHANNEL0\|pub const COMMAND" hal/drivers/pit/src/lib.rs
22:pub const CHANNEL0: u16 = 0x40;
23:pub const COMMAND: u16 = 0x43;
```

The 23 is the number of test suites (unit tests, integration tests and doc tests) in the `hal/` workspace; added up at the same commit they ran 179 tests, 0 failed.

**Exercises.** (1) Without running anything, work out the PIT divisor for 1,000 Hz and the bytes written to ports `0x40` and `0x43`, then check them against `the_chip_model_sees_mode_2_and_the_reload_value` in `hal/drivers/pit/src/lib.rs`. (2) Decode the packet `2A 03 FD` by hand: which button, and how far in which direction does the Desk's cursor move? Then find the test that would catch you if you got the sign of dy wrong. (3) The vector of IRQ 6 under the kernel's bases and under the BIOS's: say which is which, then find the row in the table above. (4) In a scratch branch, change `REMAPPED_BASES` to `(0x20, 0x20)` and say which check refuses it first.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: ISA bus
def: The 16-bit expansion bus of the IBM PC/AT. On a 486-class PC the legacy devices (timer, interrupt controllers, keyboard controller, serial ports, VGA, floppy) answer on it.

term: Port I/O
def: Access to a device through the processor's separate 64 KiB I/O address space, using the in and out instructions, as opposed to memory-mapped access. Each access really happens, once and in order.

term: IRQ
def: Interrupt request: one of the sixteen hardware interrupt lines of a PC/AT, numbered 0 to 15. The interrupt controllers turn a line into a vector number for the processor.

term: PIC
def: Programmable interrupt controller: the Intel 8259. A PC/AT has two, a master for IRQ 0 to 7 and a slave for IRQ 8 to 15 cascaded through the master's IRQ 2.

term: EOI
def: End of interrupt: the command byte 0x20 a handler sends to an interrupt controller so that it will deliver that line again. A slave line needs it on both controllers.

term: PIT
def: Programmable interval timer: the Intel 8254, whose channel 0 divides a 1,193,182 Hz clock by a programmable divisor and raises IRQ 0.

term: 8042
def: The keyboard controller of the PC/AT, a small microcontroller with a data port at 0x60 and a status and command port at 0x64. On PS/2 machines it also carries the mouse.

term: Scancode
def: The byte or bytes a keyboard sends for a key press or release. The Desk reads set 1, in which a release is the press code with bit 7 set and 0xE0 prefixes the extended keys.

term: UART
def: Universal asynchronous receiver-transmitter: the chip behind a serial port. The PC's is the 16550 or a compatible, with eight registers from a base port such as 0x3F8 for COM1.

term: VGA
def: Video graphics array: IBM's 1987 display adapter, a bank of registers on ports 0x3C0 to 0x3DA with 256 KiB of video memory in four planes, which later cards imitate.

term: Mode 13h
def: The VGA graphics mode of 320 by 200 pixels with 256 colours, one byte per pixel in a single linear block at 0xA0000.

term: Planar mode
def: A VGA mode, such as mode 12h (640 by 480, 16 colours), in which each pixel's bits are spread across four planes at the same address and drawing selects planes through registers.

term: DAC
def: Digital-to-analogue converter: the 256-entry colour table inside a VGA card. A pixel is an index into it, and each entry is six bits each of red, green and blue.

term: VBE
def: VESA BIOS Extensions: a BIOS interface, INT 10h with AH=4Fh, for listing, setting and describing the higher video modes of super-VGA cards.

term: LFB
def: Linear frame buffer: the whole screen as one flat block of memory at a physical address, as VBE 2.0 offers, instead of a 64 KiB window at 0xA0000 moved by bank switching.
```

```figures
id: m3-ports
kind: diagram
track: shared
title: The ports and interrupt lines the code names
alt: A drawing of the two 8259 interrupt controllers with their IRQ lines and the vectors the kernel gives them, above tables of the I/O ports and IRQ lines read from the driver constants.
caption: Ports and IRQ lines, read from the drivers' constants, with the kernel and BIOS vectors computed from the base addresses.
at: explainer m-ports
thumb: specimen
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every port and IRQ number named was checked against the driver constant or the assembly that uses it, and the commands above were run on the host. One disagreement between a document and the code: `docs/HAL.md` ("Console and debug output") lists a VGA text console `desk-vgatext` as an implementer of `DebugOut`, but no such crate exists in `hal/` and the kernel's text console is `kernel/src/vga.rs` writing to `0xB8000` directly (`docs/C-INPUT.md` says plainly that `vga.rs` was left alone, and `docs/HAL.md` marks the move optional). Also noted: `docs/A-VGA-VBE.md`'s status block says QEMU and the native kernel are not verified, which the later `docs/D-NATIVE-VGA.md` and `docs/N3-BIOS-PATH.md` superseded.
