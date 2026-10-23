---
id: std
n: 13
title: A std of our own?
date: 2026-10-23
summary: Spike E asked whether Rust's standard library can be built for an operating system whose name we invent. It cannot, until four missing pieces exist, and then a very small patch makes it work.
prs: [12]
---
Everything the archive has built so far is `no_std`: the kernel, the runtime, the drivers. That is right for code that has to be small and close to the hardware, and wrong for the apps that will one day run on the Desk, which would like `println!`, `HashMap` and `Instant`, and the crates that assume them. Since the project writes its own target specifications, spike E (pull request #12, {{pr12.state}}, branch `spike/e-std-stubs`, built on the pure-Rust runtime of chapter 8) asked a cheap question first: if the target's `os` is a name we make up, what does Rust's `std` give us?

**It does not build.** The expectation was that an unknown operating system falls back to the "unsupported" platform in full. In Rust 1.98.0 it does not: `std` fails with 21 errors for `none`, `desk` and `dos` alike, because four pieces have no fallback arm. Other names fare differently, and the table is the spike's.

<!-- table std-os-results.md -->

The 21 errors collapse into four missing pieces, and each is a place where the library's platform selection runs out of cases.

<!-- table std-missing.md -->

**What the stubs give you.** The spike made the smallest patch that lets an unknown name build, and ran the same probe program on three layers: the bare stubs, a borrowed layer (stock `std` with the OS set to `trusty`, whose layer calls `malloc` and `writev`, which a shim supplied), and a patched `desk` layer. The result is poor and predictable.

<!-- table std-api.md -->

The surprising row is `thread::current()`. A thread handle is allocated with the *system* allocator, not the program's own, so a stub allocator that returns null makes every panic that names its thread abort. A platform's system allocator has to work even when the program installs its own.

**A small patch.** The cheapest route that works is a patched copy of the library sources: the environment variable `__CARGO_TESTS_ONLY_SRC_ROOT` points cargo's `-Zbuild-std` at a copy of the toolchain's `library` directory, so the pinned toolchain stays untouched and there is nothing to prebuild. The patch adds 55 lines in 12 existing files, about 340 lines of new files, and a shim of about 330 lines over the runtime that implements 15 `desk_*` C functions: memory, reading and writing, arguments and environment, a seed, a monotonic clock (the BIOS tick with the PIT's counter, 838 ns steps), a wall clock from DOS and a sleep. With it, `println!`, `eprintln!`, `stdin`, `Vec`, `HashMap` with a real seed, `Instant`, `SystemTime`, `thread::sleep`, `env::args`, `env::var` and `process::exit(7)` all work in DOSBox. The same 15 functions would sit behind system calls on the native Desk.

The borrowed-layer shortcut is the trap in the table's third column: it runs, and it chooses the layer by an OS name that happens to be convenient, so time, arguments and `exit` stay stubs and a future change upstream moves the contract under the project.

**What it costs.** Size, first. One hello program, five platform layers:

<!-- figure std-sizes -->

The recommendation in the document follows the numbers. The kernel, runtime, drivers and the Desk shell stay `no_std` with `alloc`. Desk apps, and DOS programs that want the crates ecosystem, use the patched layer. A thin `desk_std` over `core` and `alloc` stays the answer for size-critical programs. An upstream target is a long-term idea only.

**What it will cost to keep.** The patch was dry-run against the library sources of other stable releases: it applies cleanly on 1.98.0, applies with offsets on 1.97.0, and applies only partly on 1.96.1 down to 1.93.0, where the lists of operating systems in the platform selection have moved. Whether the new files compile there was not tried. The expected cost is one hand rebase per Rust bump, and `check.sh` already fails if the patch stops applying.

The review found one real bug worth telling: the BIOS tick count restarts at midnight, and the clock's never-go-backwards clamp then froze it, so a `thread::sleep` spin could never end. The fix treats a fall of more than half a day as midnight. It was reasoned through; no run crossed midnight.

> **From the stacks — What a Tier 3 target is.** The Rust project sorts the platforms it supports into tiers. Its documentation says Tier 3 targets are those which the codebase has support for, but which the project does not build or test automatically, so they may or may not work, and official builds are not available. A new operating system would start there, and the spike's patch is small enough that proposing it upstream is a conceivable later step, but nothing in this repository depends on that.
> Source: [Platform Support, The rustc book](https://doc.rust-lang.org/rustc/platform-support.html)

## The shelves

### Catalogued
- With the patched layer, every API in the table's last column marked as working was run in headless DOSBox Staging 0.83 with CWSDPMI, from `spikes/e-std-stubs/check.sh`, which is a step of `tools/check.sh`; CI passed on the merge commit `{{pr12.head}}` of pull request #12, which is {{pr12.state}}.
- Building `std` for an invented OS name without the patch fails with 21 errors, as the first table says, and `check.sh` includes negative builds that expect the unpatched layers to fail.
- The sizes in the chart are measured from builds under the pinned toolchain, and the figures are the document's.

### Uncatalogued
- Real hardware: the clock and everything else ran in an emulator.
- Rust releases other than 1.98.0: the patch applies partly on the older ones and the overlay files were not compiled there.
- A run across midnight, which the clock fix was reasoned for and not run.
- `std::fs` and threads are not implemented, `env::set_var` and `remove_var` return `Unsupported` (which `std` turns into a panic), and a program that returns from the entry point skips `std`'s exit clean-up, so output without a newline is lost until `process::exit`.
- The cost of `std` on a 486: the sizes are bytes, not speed.

### Wish list
- `fn main` as the entry, letting `std`'s own start-up run so the main thread has a name and the arguments are passed in.
- `std::fs` over the runtime's DOS file calls.
- A CI job that applies the patch to the next Rust release before the toolchain is bumped.
- The same 15 functions behind system calls on the native Desk, and an upstream Tier 3 target.

```figures
id: std-sizes
kind: chart
track: hosted
title: What std costs in bytes
alt: Grouped bars of the executable size of one hello program on five platform layers under three sets of compiler flags, with the byte count at the end of each bar.
caption: A patched std adds 9.7 KB to a hello with default flags.
at: figure std-sizes
thumb: svg
```
