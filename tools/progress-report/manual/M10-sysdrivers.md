---
id: m-sysdrivers
n: M10
part: IV
title: Drivers I: serial, interrupt controllers and the timer
summary: The 16550 driver, the two-8259 driver and the 8254 driver with PitClock: their register sequences word by word, their tests against the mock bus, and how the kernel and the DOS runtime each use them.
date: 2026-10-23
reviewed: 44f0265
watches: [hal/drivers/serial/, hal/drivers/pic/, hal/drivers/pit/, kernel/src/serial.rs, kernel/src/pic.rs, kernel/src/pit.rs, kernel/src/hal.rs, spikes/d0-rust-dos/rt/src/irq.rs, docs/HAL.md]
log: [hal]
anatomy: [desk:drivers]
---
## What it is

Three small crates are the first drivers written against [the HAL](chapter:m-hal), and the ones every boot uses. `desk-serial` is the 16550 [UART](term:uart) behind COM1, the kernel's console. `desk-pic` is the pair of 8259 interrupt controllers ([PIC](term:pic)). `desk-pit` is the 8254 timer ([PIT](term:pit)) and `PitClock`, the monotonic clock it drives. Each is a single `lib.rs` with its tests below it, generic over a `PortIo` bus, with no `unsafe` and no `alloc`. Together they are about 850 lines, tests included.

On the map ([M1](chapter:m-map)) they are the `PIT` and `PIC` entries of the `drivers` box of the native track's anatomy page. [M3](chapter:m-devices) explained the chips and their ports; [M6](chapter:m-interrupts) followed the kernel's side of the interrupt and timer steps. This chapter is the drivers' side: what each function writes to which port in which order, why each choice was made, what the tests pin, and which platform calls which function. After it you should be able to read any of the three `lib.rs` files, predict the port writes of `init` for a given rate or mask, write a test against the mock bus for a change, and say what the DOS runtime does with each driver (the answer is not the same as the kernel's, and for two of them it is nothing).

The three share one design, which the file headers state. Each was written to reproduce the native kernel's earlier code exactly (the kernel's own `serial.rs`, `pic.rs` and `pit.rs` before they were replaced), and its test spells out the kernel's port writes by hand and demands the driver make the same ones. Beyond that, each adds what a second platform needs: a refused argument in place of a silent truncation, a probe, a restore of the BIOS rate, a read of the in-service register.

## How it works

### The 16550: `desk-serial`

[`Serial16550<P>`](sym:hal/drivers/serial/src/lib.rs::Serial16550) holds a bus and a base port and nothing else. `Serial16550::new` touches no hardware and is a `const fn`, so a driver can be a `static`; it asserts `base <= 0xFFF8` because the chip spans eight ports, which is an error at compile time in a `const` or `static`. Four standard bases are named: `COM1` `0x3F8`, `COM2` `0x2F8`, `COM3` `0x3E8` and `COM4` `0x2E8`. The registers are offsets from the base, and the driver names the ones it uses: data (0), interrupt enable (1), FIFO control (2), line control (3), modem control (4) and line status (5). The first two double as the divisor latch when line control bit 7 is set.

A serial line sends bits at a fixed rate. The [baud rate](term:baud-rate) is how many signal changes a second the line carries; for these chips, with 8 data bits, no parity and one stop bit, each byte is ten bits, so 115,200 baud is 11,520 bytes a second. The UART derives the rate by dividing its clock, which is 16 times 115,200, by a 16-bit number in the [divisor latch](term:divisor-latch): two registers that sit behind the data and interrupt-enable ports while bit 7 of line control (DLAB) is set. `MAX_BAUD` is 115,200, the rate at a divisor of 1.

`divisor_for(baud)` returns the divisor or `HalError::InvalidArgument`. It accepts a rate only if it is non-zero, at most 115,200, divides 115,200 exactly and gives a divisor that fits 16 bits. So 115,200 gives 1, 9,600 gives 12, 2 gives 57,600, and 0, 1, 7, 100,000 and 230,400 are all refused. The reason is in the doc comment: a rate that does not divide exactly would be rounded to a rate "the other end does not expect", and a silent wrong rate is worse than an error. (Rate 1 is refused because its divisor, 115,200, does not fit in 16 bits; the slowest rate accepted is 2.)

`init(baud)` programs the chip in this order, seven writes, after computing the divisor (a refused rate returns before any write):

| Step | Port (COM1) | Value | What it does |
|---|---|---|---|
| 1 | `0x3F9` interrupt enable | `0x00` | All UART interrupts off: the driver is polled |
| 2 | `0x3FB` line control | `0x80` | DLAB on: ports 0 and 1 now hold the divisor |
| 3 | `0x3F8` | divisor low byte | `1` at 115,200 baud |
| 4 | `0x3F9` | divisor high byte | `0` |
| 5 | `0x3FB` line control | `0x03` | DLAB off, 8 data bits, no parity, one stop bit |
| 6 | `0x3FA` FIFO control | `0xC7` | Enable the FIFOs, clear both, 14-byte receive trigger (bits 7 and 6 set) |
| 7 | `0x3FC` modem control | `0x0B` | DTR, RTS and OUT2 set; on a PC OUT2 gates the UART's interrupt line onto the bus |

The test `init_matches_the_kernel_byte_for_byte` holds exactly this list as `KERNEL_INIT`, a constant of seven `(port, value)` pairs, and runs `init(115_200)` against a bus scripted with them: a different value, order or an eighth write fails.

`write_byte(byte)` polls line status (offset 5) up to `WRITE_POLLS`, 100,000 times, for bit 5 (transmitter holding register empty), and then writes the byte whether or not the transmitter came free. That is the kernel's behaviour kept on purpose: a missing or stuck UART costs a bounded amount of time and loses output, and never hangs the machine, which matters in a panic handler. The test `a_stuck_transmitter_costs_a_bounded_wait_and_the_byte_is_still_written` sets the model's `busy` flag and counts exactly 100,000 status reads before the data write. `the_wait_ends_as_soon_as_the_transmitter_is_free` scripts three status reads (`0x00`, `0x00`, `0x20`) and shows the loop stops at the third. `Serial16550` implements `core::fmt::Write` (so `writeln!` works) and the HAL's `DebugOut`.

`read_byte()` returns `Some(byte)` if line status bit 0 (data ready) is set and `None` otherwise; it never blocks. `probe()` asks whether a UART answers at all. The chip has a loopback mode (modem control bit 4) in which a transmitted byte returns to the receiver. `probe` waits boundedly for the transmitter to be idle (status bit 6, so that a byte already leaving is not looped back and lost), sets modem control to `0x1E` (loopback, with OUT1, OUT2 and RTS), drains up to 16 stale bytes (`RX_FIFO`), sends `0xAE`, polls up to `WRITE_POLLS` times for it to come back, skipping at most a FIFO's worth of other bytes, and restores modem control to `0x0B` before it returns `Ok` or `NoDevice`. A floating bus reads all ones, which looks ready but echoes `0xFF` and not `0xAE`, so an empty slot is told from a chip. The echo takes one character time, ten bits at 115,200 baud, about 87 microseconds.

Interrupt-driven transmission and reception are not built; the header says they will come "as an `IrqService` feeding an `SpscRing`". The FIFO is turned on and its trigger level set, but nothing reads it by interrupt.

> **From the stacks — Why the FIFO.** Wikipedia's article on the 16550 UART says that the earlier 8250 and 16450 generated an interrupt for each byte received, and that the 16550 series added a 16-byte FIFO buffer with a programmable interrupt trigger of 1, 4, 8 or 14 bytes. It says the original 16550 had a bug that prevented the FIFO from being used, and that National Semiconductor released a corrected 16550A in 1987.
> Source: [16550 UART, Wikipedia](https://en.wikipedia.org/wiki/16550_UART)

### The two 8259s: `desk-pic`

[`Pic8259<P>`](sym:hal/drivers/pic/src/lib.rs::Pic8259) is a bus and nothing else; its two controllers are at fixed ports, `MASTER` `0x20` and `SLAVE` `0xA0`, each with a command port and a data port one higher. The same struct is used by both platforms, but only the native one calls `init`.

The chip is programmed by a sequence of command bytes called initialisation command words, [ICW](term:icw)s, sent in order to the command port (ICW1) and then the data port (ICW2 to ICW4). `init(bases, mask)` sends eight words, each followed by `io_wait` (a write to the POST port, about a microsecond), because the 8259 on old machines cannot take them back to back, and then the two mask bytes:

| Word | To the master | To the slave | Meaning |
|---|---|---|---|
| ICW1 | `0x20` ← `0x11` | `0xA0` ← `0x11` | Start initialising; edge-triggered, cascaded, an ICW4 will follow |
| ICW2 | `0x21` ← base | `0xA1` ← base | The vector of the first line: `0x20` and `0x28` for `REMAPPED_BASES` (the BIOS's `BIOS_BASES` are `0x08` and `0x70`) |
| ICW3 | `0x21` ← `0x04` | `0xA1` ← `0x02` | Wiring: the master has a slave on line 2 (bit 2); the slave's identity is 2 |
| ICW4 | `0x21` ← `0x01` | `0xA1` ← `0x01` | 8086 mode |
| Masks | `0x21` ← mask low byte | `0xA1` ← mask high byte | A set bit blocks the line |

The order in code is ICW1 to the master, ICW1 to the slave, ICW2 to the master, ICW2 to the slave and so on, which is the `steps` array in `init`. The mask is a 16-bit value, bit n for IRQ n, so `0xFFFE` writes `0xFE` and `0xFF`: only the timer is open. The test `native_init_matches_the_kernel_with_its_delays` writes the eight pairs out by hand, interleaves each with `(0x80, 0)`, appends `(0x21, 0xFE)` and `(0xA1, 0xFF)`, turns on `record_post_delays` so the mock bus keeps the delays, and requires the whole script to be consumed. `set_mask` is the last step alone, `mask_bits` reads both registers back, and `init` is documented native only: under DOS it "would take the interrupts away from the BIOS".

The rest implements the HAL's `InterruptController` trait:

- `mask`, `unmask`, `is_masked` read the controller's mask register, change one bit and write it back. `unmask` of a slave line also unmasks the cascade (`Irq::CASCADE`), without which a slave line cannot reach the processor. The test `masking_reads_modifies_and_writes_one_bit` scripts the seven accesses of `mask(TIMER)`, `unmask(MOUSE)` and `is_masked(MOUSE)`: read `0xFE`, write `0xFF`; read `0xFF` from the slave, write `0xEF`; read `0xFF`, write `0xFB` on the master (the cascade); read `0xEF`. These are read-modify-write sequences, so the caller must hold the platform's critical section; the trait's comment says so.
- `eoi` writes `0x20` (`OCW2_EOI`) to the slave first, if the line is 8 or more, and always to the master. `a_slave_line_is_acknowledged_on_both_controllers` checks that the keyboard gets `[(0x20, 0x20)]` and the mouse `[(0xA0, 0x20), (0x20, 0x20)]`.
- `is_spurious` is only true for lines 7 and 15: it writes `0x0B` (`OCW3_READ_ISR`) to the controller's command port and reads the in-service register back; if bit 7 is clear, no real interrupt is in service and this one is [spurious](term:spurious-interrupt). `eoi_spurious` does nothing for a spurious IRQ 7 and sends the master `0x20` for IRQ 15 (`eoi_master`), because the master saw its cascade input. The tests script both.
- `in_service_mask` and `pending_mask` read both in-service registers (OCW3 `0x0B`) or both interrupt-request registers (OCW3 `0x0A`) as one 16-bit value. `the_service_and_request_registers_are_read_through_ocw3` shows `0x1001` for IRQ 0 and IRQ 12 in service and `0x0002` for IRQ 1 waiting. These two were added by the BIOS-call spike (`docs/N3-BIOS-PATH.md`, the HAL feedback) and the kernel's `bios.rs` is their only caller: before switching the controllers to the BIOS's bases it reads `pending_mask` to know whether a tick is waiting that re-initialising would throw away, and it uses `in_service_mask` to refuse a BIOS call from interrupt context.

### The 8254 and `PitClock`: `desk-pit`

[`Pit<P>`](sym:hal/drivers/pit/src/lib.rs::Pit) programs channel 0 at port `0x40` through the command port `0x43`. The input clock is `INPUT_HZ`, 1,193,182 Hz. The command byte is `CMD_CH0_LOHI_RATE`, `0x34`, which is `00 11 010 0` in binary: channel 0, the 16-bit divisor sent low byte then high byte, mode 2 (rate generator), binary counting. The BIOS's byte is `0x36`, the same with mode 3 (square wave), and the counter-latch command is `0x00`.

- `divisor_for(hz)` computes `(INPUT_HZ + hz / 2) / hz`, a division rounded to the nearest, and passes the result through `check_divisor`, the one copy of the range rule: a divisor must be from 2 (`MIN_DIVISOR`, because mode 2 needs at least 2) to 65,536 (`MAX_DIVISOR`). In whole hertz that leaves 19 Hz (divisor 62,799) to 795,454 Hz (divisor 2); 18 Hz, which needs 66,288, and 795,455 and above are refused. The kernel's old code truncated; this refuses, and the test `rates_the_chip_cannot_produce_are_refused` lists `0, 1, 18, 795_455, 1_193_182, u32::MAX`.
- `program_divisor(divisor)` writes the three bytes: `0x34` to `0x43`, the low byte, then the high byte, both to `0x40`. A divisor of 65,536 is written as zero, which is how the chip encodes it (`% MAX_DIVISOR`). `program_periodic(hz)` computes the divisor and calls it, writing nothing if the rate is out of range, and `program_clock(&clock, hz)` does that and records the divisor in a `PitClock` in the same step, so the clock's rate cannot differ from the chip's.
- `restore_bios_rate()` writes `0x36` to `0x43` and two zero bytes to `0x40`: what the BIOS had, mode 3 and divisor 65,536, about 18.2 Hz. It exists for the hosted platform to run on exit; nobody calls it yet.
- `read_counter()` latches channel 0 (`0x00` to `0x43`), then reads the low and high bytes from `0x40`. In mode 2 the count falls by one per input clock, 838 nanoseconds, so `divisor - count` is the time since the last tick, which is how sub-tick time would be had on a CPU with no time-stamp counter. The doc comment says it must be run with interrupts off and that in the BIOS's mode 3 the count falls by two and covers each period twice, so the hosted track would reprogram to mode 2 first. Nothing calls it yet either.
- `actual_millihertz(divisor)` is the rate the divisor really gives, rounded to the nearest thousandth of a hertz: 99,998 for 11,932, 18,207 for 65,536 and 1,000,153 for 1,193. It returns zero for a divisor of 0, which is not a rate.

[`PitClock`](sym:hal/drivers/pit/src/lib.rs::PitClock) is the `Clock` the core reads. It contains a HAL `TickCounter` and an `AtomicU32` for the rate, and lives in a `static`. `counter()` returns the `TickCounter`, which is the IRQ 0 work: the platform registers `IrqHandler::from_shared(CLOCK.counter())` and every interrupt does one `fetch_add`. `now()` reads the counter and `millihertz()` the recorded rate (zero until `set_divisor` or `program_clock` records one). A clock whose rate is zero reports zero milliseconds, so a clock that was never programmed cannot be mistaken for one that is running.

### How each platform uses them

| Driver | Native kernel (`kernel/src/`) | DOS runtime (`spikes/d0-rust-dos/rt/src/`) |
|---|---|---|
| `desk-serial` | `static SERIAL: Serial16550<RawPorts>` in `hal.rs`; `serial::init` calls `SERIAL.init(115_200)`; the `Serial` type in `serial.rs` is a `fmt::Write` that calls `write_byte`, and `console.rs` writes every line to it and to the screen | Not used. The hosted program's output is standard output |
| `desk-pic` | `static PIC` in `hal.rs`; `pic::init` calls `PIC.init(REMAPPED_BASES, 0xFFFE)`; `IrqTable::register` calls `unmask`; `irq_dispatch` calls `is_spurious`, `eoi_spurious` and `eoi`; `bios.rs` calls `init(BIOS_BASES, …)` and back, `mask_bits`, `pending_mask` and `in_service_mask` | `rt_irq_dispatch` makes a `Pic8259` over `RawPorts` and calls `is_spurious`, `eoi_spurious` and `eoi` for a line Desk owns; `DosIrq::register` calls `is_masked` and `unmask`. It never calls `init`: the BIOS owns the controllers at `0x08` and `0x70` |
| `desk-pit` | `static CLOCK: PitClock`; `pit::init` calls `Pit::new(PORTS).program_clock(&CLOCK, hz)` and registers `IrqHandler::from_shared(CLOCK.counter())` on IRQ 0 with `Chain::None`; `pit::ticks` reads `CLOCK.now().0` | Not used. `rt::timer` hooks IRQ 0 with its own assembly stub that counts ticks and chains to the BIOS handler |

So the three drivers are fully shared only on the native track. On the hosted track the PIC driver is shared and the other two are not, although the design intends them to be (`docs/HAL.md`, "The platforms": a `PitClock` at 18,207 mHz and `restore_bios_rate` on exit). The kernel side of every row is taught in [M6](chapter:m-interrupts) and the boot transcript lines they print are in [M5](chapter:m-boot).

## Why it is this way

- **Reproduce the kernel, byte for byte, then extend.** The first requirement of each driver was that the kernel could drop its own copy without changing a transcript line. The tests prove the port writes equal the old ones, written out by hand (`docs/HAL.md`, "What was and was not verified": equivalence "by tests against the kernel's values written out by hand, not by running the kernel against the mock"). The proof that the kernel still boots with the shared copies is the QEMU transcript, which does not change (`tests/native/expected.txt`).
- **Refuse, do not round.** `divisor_for` in both `desk-serial` and `desk-pit` returns `InvalidArgument` for what the chip cannot do. The PIT's refusal and the UART's exact-division rule are instances of the rule that "out-of-range device arguments are `InvalidArgument` and write nothing" (`docs/HAL.md`, "Errors").
- **Bounded waits, and carry on.** A status poll has a bound (`WRITE_POLLS`), and the serial driver writes the byte anyway, as the kernel did, because a panic message that costs 100,000 reads is better than a silent hang. `docs/HAL.md` ("Errors") says an 8042 command, by contrast, returns `Timeout`, because carrying on there would be wrong.
- **The driver never sends the end-of-interrupt.** The kernel's old `pit::tick` did; under DOS the chained BIOS handler sends it for the timer, and a second one would acknowledge a different interrupt (`docs/HAL.md`, "Interrupts"). So `PitClock::counter` only counts and the platform's dispatch sends the EOI. The test `ticks_arrive_through_a_registered_handler` shows 25 end-of-interrupts sent by the platform.
- **A clock that records its own rate.** `program_clock` exists so that the divisor and the clock's rate are set together. A caller that programmed the chip and forgot the clock would report seconds at the wrong speed.
- **Millihertz.** The rate is kept in thousandths of a hertz because "100 Hz" is really 99.998 and the BIOS's 18.2 is 18.207; an integer number of hertz would make the clock drift visibly or lose the BIOS rate.
- **A gap in the verification of the registers.** The mock models are the author's reading of the data sheets. A `Uart16550` model does not model the FIFO depth or interrupts, and nothing has compared any model with a real chip. QEMU, Bochs and 86Box emulate the chips, so a boot there tests that the driver's writes bring up an emulator's chip, which is a different claim from a real 486's.

## The code

| Where | What it holds |
|---|---|
| [`hal/drivers/serial/src/lib.rs`](repo:hal/drivers/serial/src/lib.rs) | [`Serial16550`](sym:hal/drivers/serial/src/lib.rs::Serial16550) with `init`, `probe`, `write_byte`, `read_byte`; [`divisor_for`](sym:hal/drivers/serial/src/lib.rs::divisor_for); [`COM1`](sym:hal/drivers/serial/src/lib.rs::COM1) to `COM4`; [`MAX_BAUD`](sym:hal/drivers/serial/src/lib.rs::MAX_BAUD); [`WRITE_POLLS`](sym:hal/drivers/serial/src/lib.rs::WRITE_POLLS); eight tests |
| [`hal/drivers/pic/src/lib.rs`](repo:hal/drivers/pic/src/lib.rs) | [`Pic8259`](sym:hal/drivers/pic/src/lib.rs::Pic8259) with `init`, `set_mask`, `mask_bits`, `in_service_mask`, `pending_mask` and the `InterruptController` impl; [`MASTER`](sym:hal/drivers/pic/src/lib.rs::MASTER), [`SLAVE`](sym:hal/drivers/pic/src/lib.rs::SLAVE), [`BIOS_BASES`](sym:hal/drivers/pic/src/lib.rs::BIOS_BASES), [`REMAPPED_BASES`](sym:hal/drivers/pic/src/lib.rs::REMAPPED_BASES); six tests |
| [`hal/drivers/pit/src/lib.rs`](repo:hal/drivers/pit/src/lib.rs) | [`Pit`](sym:hal/drivers/pit/src/lib.rs::Pit), [`PitClock`](sym:hal/drivers/pit/src/lib.rs::PitClock), [`divisor_for`](sym:hal/drivers/pit/src/lib.rs::divisor_for), [`check_divisor`](sym:hal/drivers/pit/src/lib.rs::check_divisor), [`actual_millihertz`](sym:hal/drivers/pit/src/lib.rs::actual_millihertz), [`INPUT_HZ`](sym:hal/drivers/pit/src/lib.rs::INPUT_HZ); eight tests |
| [`hal/platform/mock/src/devices.rs`](repo:hal/platform/mock/src/devices.rs) | The [`Uart16550`](sym:hal/platform/mock/src/devices.rs::Uart16550) and [`Pit8254`](sym:hal/platform/mock/src/devices.rs::Pit8254) models the tests attach |
| [`kernel/src/serial.rs`](repo:kernel/src/serial.rs), [`pic.rs`](repo:kernel/src/pic.rs), [`pit.rs`](repo:kernel/src/pit.rs) | The kernel's thin wrappers: [`serial::init`](sym:kernel/src/serial.rs::init), [`pic::init`](sym:kernel/src/pic.rs::init), [`pit::init`](sym:kernel/src/pit.rs::init) |
| [`kernel/src/hal.rs`](repo:kernel/src/hal.rs) | The statics [`PIC`](sym:kernel/src/hal.rs::PIC), [`SERIAL`](sym:kernel/src/hal.rs::SERIAL) and [`CLOCK`](sym:kernel/src/hal.rs::CLOCK) |
| [`spikes/d0-rust-dos/rt/src/irq.rs`](repo:spikes/d0-rust-dos/rt/src/irq.rs) | The DOS runtime's use of `Pic8259`, in [`rt_irq_dispatch`](sym:spikes/d0-rust-dos/rt/src/irq.rs::rt_irq_dispatch) |
| [`tools/progress-report/tests/test_chapter_hal_drivers.py`](repo:tools/progress-report/tests/test_chapter_hal_drivers.py) | The checks of this chapter and of M9 against the sources |

For depth: `docs/HAL.md` ([interrupts](doc:docs/HAL.md#interrupts-built), [time](doc:docs/HAL.md#time-built-the-wall-clock-is-a-sketch), [errors](doc:docs/HAL.md#errors)) and `docs/N3-BIOS-PATH.md` for the two register reads.

## Worked example

**Three sequences, from three tests.** Each is the real list in the driver's own test.

*The UART at 9,600 baud on COM2.* `the_divisor_follows_the_rate` attaches a `Uart16550` model at `COM2` (`0x2F8`) and calls `init(9_600)`. The divisor is 115,200 ÷ 9,600 = 12 (`0x0C`). The seven writes are `0x2F9`←`0x00`, `0x2FB`←`0x80`, `0x2F8`←`0x0C`, `0x2F9`←`0x00`, `0x2FB`←`0x03`, `0x2FA`←`0xC7`, `0x2FC`←`0x0B`. The test then reads the model and expects `(divisor, lcr, fcr, mcr, ier)` to be `(12, 0x03, 0xC7, 0x0B, 0)`: the latch took `0x0C` low and `0x00` high while DLAB was on, and line control ended at 8N1. It also calls `init(100_000)` on the same bus and checks the log still holds seven writes: the refused rate wrote nothing. The `0xC7` is `11 000 1 1 1`: trigger level bits `11` (14 bytes), the transmit-clear bit, the receive-clear bit and the enable bit.

*The 8259s, remapped.* `native_init_matches_the_kernel_with_its_delays` expects, in order, `(0x20,0x11) (0x80,0)`, `(0xA0,0x11) (0x80,0)`, `(0x21,0x20) (0x80,0)`, `(0xA1,0x28) (0x80,0)`, `(0x21,0x04) (0x80,0)`, `(0xA1,0x02) (0x80,0)`, `(0x21,0x01) (0x80,0)`, `(0xA1,0x01) (0x80,0)`, then `(0x21,0xFE)` and `(0xA1,0xFF)`: sixteen writes for the eight words and their delays, and two for the masks. The vector of IRQ 12, the mouse, is the slave's base plus 4, `0x28 + 4 = 0x2C`; under the BIOS's bases it would be `0x70 + 4 = 0x74`. A spurious IRQ 7 from the master with the register read as `0x00` is told apart from a real one that reads `0x80`.

*The PIT, restored.* `the_chip_model_sees_mode_2_and_the_reload_value` programs 1,000 Hz: the divisor is (1,193,182 + 500) ÷ 1,000 = 1,193 (`0x04A9`), so the model's channel 0 holds access 3, mode 2, binary, reload 1,193. It then calls `restore_bios_rate` and expects mode 3 and a reload of 0 (the encoding of 65,536), and the bus log from the fourth write on is `(0x43, 0x36)`, `(0x40, 0)`, `(0x40, 0)`. The clock for 1,193 would be `actual_millihertz(1_193)` = 1,000,153, a hair over one kilohertz.

## State of this layer

```status
id: drv.serial-init
label: Serial16550::init writes the kernel's seven values and the kernel's console runs on it
state: built-verified
evidence: test:hal/drivers/serial/src/lib.rs::init_matches_the_kernel_byte_for_byte, test:tests/native/expected.txt::Desk native kernel 0.1.0 (i486)
note: Host test against the mock bus, and the kernel's boot transcript in QEMU as a 486 (rerun for this chapter). QEMU emulates the chip; no real UART was driven.

id: drv.serial-wait
label: The bounded transmit wait, the byte still written after 100,000 polls
state: built-verified
evidence: test:hal/drivers/serial/src/lib.rs::a_stuck_transmitter_costs_a_bounded_wait_and_the_byte_is_still_written, sym:hal/drivers/serial/src/lib.rs::WRITE_POLLS
note: Host only. The timing of a real stuck transmitter is not measured.

id: drv.serial-probe
label: probe and read_byte
state: built-verified
evidence: test:hal/drivers/serial/src/lib.rs::probe_drains_stale_bytes_and_waits_for_the_echo, test:hal/drivers/serial/src/lib.rs::received_bytes_are_read_without_blocking
note: Host tests against the mock only; no platform calls either function, so neither has run in an emulator.

id: drv.serial-irq
label: Interrupt-driven serial transmission and reception
state: planned
evidence: doc:docs/HAL.md#console-and-debug-output-built-debugout-and-the-16550

id: drv.pic-init
label: Pic8259::init with its eight delayed words and the masks
state: built-verified
evidence: test:hal/drivers/pic/src/lib.rs::native_init_matches_the_kernel_with_its_delays, test:tests/native/expected.txt::pic: remapped to 0x20 and 0x28
note: Host test and the QEMU boot. The sequence for the BIOS's bases (used around a BIOS call) is the same code with other values and is covered by the BIOS test mode in M8.

id: drv.pic-masking
label: mask, unmask (with the cascade), is_masked and the end-of-interrupt order
state: built-verified
evidence: test:hal/drivers/pic/src/lib.rs::masking_reads_modifies_and_writes_one_bit, test:hal/drivers/pic/src/lib.rs::a_slave_line_is_acknowledged_on_both_controllers
note: Host. In QEMU the timer, keyboard and mouse lines are exercised through them.

id: drv.pic-spurious
label: is_spurious and eoi_spurious, with the in-service and request register reads
state: built-verified
evidence: test:hal/drivers/pic/src/lib.rs::spurious_interrupts_are_told_apart_by_the_in_service_register, test:hal/drivers/pic/src/lib.rs::the_service_and_request_registers_are_read_through_ocw3, test:hal/drivers/pic/src/lib.rs::a_spurious_irq_15_is_acknowledged_on_the_master_only
note: Host tests. No run reached a spurious interrupt in an emulator; M6 records the kernel's side as built-unverified.

id: drv.pit-program
label: Pit::program_periodic and program_clock, with the refusal of rates the chip cannot make
state: built-verified
evidence: test:hal/drivers/pit/src/lib.rs::programming_100_hz_writes_what_the_kernel_writes, test:hal/drivers/pit/src/lib.rs::rates_the_chip_cannot_produce_are_refused, test:tests/native/expected.txt::timer: waited for 10 ticks with hlt
note: Host tests and ten ticks counted in QEMU, which emulates the chip.

id: drv.pit-clock
label: PitClock counts ticks through a handler registered from a static
state: built-verified
evidence: test:hal/drivers/pit/src/lib.rs::ticks_arrive_through_a_registered_handler, test:hal/drivers/pit/src/lib.rs::the_actual_rate_is_reported

id: drv.pit-extras
label: restore_bios_rate and read_counter
state: built-verified
evidence: test:hal/drivers/pit/src/lib.rs::the_chip_model_sees_mode_2_and_the_reload_value, test:hal/drivers/pit/src/lib.rs::the_counter_is_latched_then_read_low_then_high
note: Host tests against the mock only; nothing calls either function on a platform.

id: drv.dos-pic
label: The DOS runtime sending EOIs and unmasking lines through Pic8259
state: built-unverified
evidence: sym:spikes/d0-rust-dos/rt/src/irq.rs::rt_irq_dispatch, repo:docs/B-DOS-INPUT.md
note: Written and recorded in the spike's document as run in headless DOSBox; not run for this chapter.

id: drv.dos-pit
label: PitClock and desk-pit under DOS (the BIOS rate as a clock, restore on exit)
state: planned
evidence: doc:docs/HAL.md#the-platforms

id: drv.hardware
label: Any of the three drivers on a real machine
state: planned
evidence: doc:docs/HAL.md#what-was-and-was-not-verified
```

The three drivers are {{state.drv.serial-init}}, {{state.drv.pic-init}} and {{state.drv.pit-program}} on the host and in QEMU. Their use under DOS is {{state.drv.dos-pic}} for the PIC and {{state.drv.dos-pit}} for the PIT; a real machine is {{state.drv.hardware}}.

## Try it yourself

The first command runs the three drivers' tests. The second boots the kernel in QEMU as a 486 and picks out the lines these drivers print; the third shows the same boot passing the transcript check. They need a Rust toolchain, NASM and `qemu-system-i386`.

```try
needs: host: bash, cargo, nasm, qemu-system-i386, tr, grep
ran: 2026-10-23 on the host at 44f0265, QEMU 11.1.2, NASM 3.02
$ cd hal && cargo test -p desk-serial -p desk-pic -p desk-pit 2>&1 | grep -E "^test result: ok. [1-9]"
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ tools/native-build.sh > /dev/null 2>&1 && tools/native-image.sh > /dev/null 2>&1 && tools/native-run.sh --serial dist/serial.txt > /dev/null 2>&1; echo "exit $?"
exit 0
$ tr -d '\r' < dist/serial.txt | grep -E "^(Desk native|pic:|pit:|timer:)"
Desk native kernel 0.1.0 (i486)
pic: remapped to 0x20 and 0x28, only IRQ0 unmasked
pit: channel 0 at 100 Hz
timer: waited for 10 ticks with hlt, elapsed 10 ticks
$ tr -d '\r' < dist/serial.txt > dist/serial.clean; python3 -I tools/native-check.py tests/native/expected.txt dist/serial.clean
transcript matches tests/native/expected.txt (38 lines)
```

Each line of the second block is one of the drivers working in an emulator: the serial driver wrote all of it, the PIC driver printed its line after `init`, and the timer line is ten interrupts counted by `PitClock`.

**Exercises.** (1) Without running it, work out the seven writes of `Serial16550::new(&bus, COM1).init(19_200)` and the divisor, then check against `divisor_for`. (2) Predict the divisor and the three bytes written for `program_periodic(60)`, and check against `the_divisor_is_rounded_to_the_nearest_as_in_the_kernel`. (3) In a scratch branch change `ICW3_SLAVE_ON_IRQ2` in `desk-pic` to `0x08` and say which test fails first and what the message shows; then change it back. (4) The kernel never calls `probe`. Write a host test that runs `probe` on a `Uart16550` model whose `busy` flag is set and say what you expect it to return, then check.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Baud rate
def: The number of signal changes a second on a serial line. With 8 data bits, no parity and one stop bit a byte is ten bits, so the 16550's top rate of 115,200 baud carries 11,520 bytes a second.

term: Divisor latch
def: The 16-bit number the 16550 divides its clock by to get the baud rate. It is two registers that appear at the data and interrupt-enable ports while bit 7 of line control (DLAB) is set; a divisor of 1 gives 115,200 baud.

term: ICW
def: Initialisation command word: one of the up-to-four bytes that set up an 8259 interrupt controller in order (the start, the vector base, the cascade wiring and the mode). The driver sends eight, four to each chip, then the masks.
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The three drivers were read in full and their tests run on the host (6, 8 and 8 passing); the native kernel was built and booted in QEMU and its transcript matched `tests/native/expected.txt`. Found while writing: the DOS runtime uses `desk-pic` but neither `desk-serial` nor `desk-pit`, although `docs/HAL.md` lists them as shared and plans a `PitClock` for the hosted platform; `restore_bios_rate`, `read_counter`, `probe` and `read_byte` have no caller outside their tests; and the header comments of `desk-serial`, `desk-pic` and `desk-pit` still name the kernel's old `serial.rs`, `pic.rs` and `pit.rs` as the reference for code those files now call the drivers with.
